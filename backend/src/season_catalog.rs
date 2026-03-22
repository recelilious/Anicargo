use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
    sync::OnceLock,
};

use chrono::{Datelike, Local, NaiveDate, Utc};
use futures::stream::{self, StreamExt};
use regex::Regex;
use sqlx::{FromRow, SqlitePool};
use tracing::warn;

use crate::{
    bangumi::{BangumiClient, BangumiSearchQuery, EpisodeRaw, SubjectRaw},
    types::{AppError, CalendarDayDto, SubjectCardDto, WeekdayDto},
    yuc::YucClient,
};

const CATALOG_REFRESH_TTL_HOURS: i64 = 12;
const MATCH_CONCURRENCY: usize = 6;
const STATUS_REFRESH_CONCURRENCY: usize = 6;

#[derive(Debug, Clone)]
struct YucCatalog {
    catalog_key: String,
    catalog_kind: String,
    season_year: i32,
    season_month: u32,
    title: String,
    source_url: String,
    source_hash: String,
    entries: Vec<YucCatalogEntry>,
}

#[derive(Debug, Clone)]
struct YucCatalogEntry {
    sort_index: i64,
    weekday: WeekdayDto,
    title: String,
    title_cn: String,
    title_original: Option<String>,
    broadcast_time: Option<String>,
    broadcast_label: Option<String>,
    episode_note: Option<String>,
    platform_note: Option<String>,
    source_image_url: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct CatalogSnapshotRow {
    source_hash: String,
    refreshed_at: String,
}

#[derive(Debug, Clone, FromRow)]
struct CatalogMatchRow {
    id: i64,
    title: String,
    title_cn: String,
    title_original: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct CalendarEntryRow {
    weekday_id: i64,
    title: String,
    title_cn: String,
    title_original: Option<String>,
    broadcast_time: Option<String>,
    bangumi_subject_id: Option<i64>,
    source_image_url: Option<String>,
    cached_title: Option<String>,
    cached_title_cn: Option<String>,
    summary: Option<String>,
    air_date: Option<String>,
    air_weekday: Option<i64>,
    image_portrait: Option<String>,
    image_banner: Option<String>,
    tags_json: Option<String>,
    total_episodes: Option<i64>,
    rating_score: Option<f64>,
    release_status: Option<String>,
}

#[derive(Debug, Clone)]
struct DetailScheduleEntry {
    title_original: String,
    broadcast_label: Option<String>,
    normalized_cn: String,
    stripped_cn: String,
}

#[derive(Debug, Clone)]
struct BangumiMatchResolution {
    subject_id: Option<i64>,
    score: Option<f64>,
    matched_title: Option<String>,
    card: Option<SubjectCardDto>,
}

impl CalendarEntryRow {
    fn to_card(&self) -> Option<SubjectCardDto> {
        let bangumi_subject_id = self.bangumi_subject_id?;
        Some(SubjectCardDto {
            bangumi_subject_id,
            title: self
                .cached_title
                .clone()
                .or_else(|| self.title_original.clone())
                .unwrap_or_else(|| self.title.clone()),
            title_cn: self
                .cached_title_cn
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| self.title_cn.clone()),
            summary: self.summary.clone().unwrap_or_default(),
            release_status: self
                .release_status
                .clone()
                .unwrap_or_else(|| "completed".to_owned()),
            air_date: self.air_date.clone(),
            broadcast_time: self.broadcast_time.clone(),
            air_weekday: self
                .air_weekday
                .and_then(|value| u8::try_from(value).ok())
                .or_else(|| u8::try_from(self.weekday_id).ok()),
            image_portrait: self
                .image_portrait
                .clone()
                .or_else(|| self.source_image_url.clone()),
            image_banner: self
                .image_banner
                .clone()
                .or_else(|| self.source_image_url.clone()),
            tags: self
                .tags_json
                .as_deref()
                .and_then(parse_tags_json)
                .unwrap_or_default(),
            total_episodes: self.total_episodes,
            rating_score: self.rating_score,
        })
    }
}

pub async fn load_current_season_calendar(
    yuc: &YucClient,
    pool: &SqlitePool,
    bangumi: &BangumiClient,
) -> Result<Vec<CalendarDayDto>, AppError> {
    let catalog_key = yuc.current_season_key();
    let sync_result = sync_current_season_catalog(yuc, pool, bangumi, &catalog_key).await;
    if let Err(error) = sync_result.as_ref() {
        warn!(
            catalog_key = %catalog_key,
            error = %error,
            "Failed to refresh Yuc season catalog; attempting to serve cached data"
        );
    }

    let rows = load_calendar_rows(pool, &catalog_key).await?;
    if rows.is_empty() {
        if let Err(error) = sync_result {
            return Err(error);
        }

        return Err(AppError::internal("current Yuc season catalog is empty"));
    }

    let mut groups = HashMap::<u8, Vec<SubjectCardDto>>::new();
    let mut skipped = 0usize;
    for row in rows {
        let weekday_id = u8::try_from(row.weekday_id).unwrap_or(0);
        let Some(card) = row.to_card() else {
            skipped += 1;
            continue;
        };

        groups.entry(weekday_id).or_default().push(card);
    }

    if skipped > 0 {
        warn!(
            catalog_key = %catalog_key,
            skipped,
            "Skipped unresolved Yuc catalog entries while building calendar response"
        );
    }

    let mut days = ordered_weekdays()
        .into_iter()
        .map(|weekday| CalendarDayDto {
            items: groups.remove(&weekday.id).unwrap_or_default(),
            weekday,
        })
        .collect::<Vec<_>>();

    for day in &mut days {
        sort_cards_by_broadcast_time(&mut day.items);
    }

    Ok(days)
}

async fn sync_current_season_catalog(
    yuc: &YucClient,
    pool: &SqlitePool,
    bangumi: &BangumiClient,
    catalog_key: &str,
) -> Result<(), AppError> {
    let snapshot = load_catalog_snapshot(pool, catalog_key).await?;
    let now = Utc::now().to_rfc3339();

    let needs_fetch = snapshot
        .as_ref()
        .is_none_or(|row| is_stale_rfc3339(&row.refreshed_at, CATALOG_REFRESH_TTL_HOURS));

    if needs_fetch {
        let html = yuc.fetch_season_html(catalog_key).await?;
        let catalog = parse_catalog(yuc, catalog_key, &html);

        if snapshot
            .as_ref()
            .is_some_and(|row| row.source_hash == catalog.source_hash)
        {
            touch_catalog_refresh(pool, &catalog, &now).await?;
        } else {
            store_catalog(pool, &catalog, &now).await?;
        }
    }

    populate_missing_matches(pool, bangumi, catalog_key).await?;
    refresh_subject_statuses(pool, bangumi, catalog_key).await?;
    Ok(())
}

fn parse_catalog(yuc: &YucClient, catalog_key: &str, html: &str) -> YucCatalog {
    let (season_year, season_month) =
        parse_catalog_key(catalog_key).unwrap_or((Utc::now().year(), 1));
    let mut entries = parse_weekday_entries(html);
    let details = parse_detail_entries(html);
    attach_detail_entries(&mut entries, &details);

    YucCatalog {
        catalog_key: catalog_key.to_owned(),
        catalog_kind: "season".to_owned(),
        season_year,
        season_month,
        title: format!("{season_year}年{season_month}月新番表"),
        source_url: yuc.season_url(catalog_key),
        source_hash: hash_text(html),
        entries,
    }
}

async fn load_catalog_snapshot(
    pool: &SqlitePool,
    catalog_key: &str,
) -> Result<Option<CatalogSnapshotRow>, AppError> {
    sqlx::query_as::<_, CatalogSnapshotRow>(
        "SELECT source_hash, refreshed_at
         FROM yuc_catalogs
         WHERE catalog_key = ?1
         LIMIT 1",
    )
    .bind(catalog_key)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::internal("failed to read Yuc catalog snapshot"))
}

async fn touch_catalog_refresh(
    pool: &SqlitePool,
    catalog: &YucCatalog,
    refreshed_at: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE yuc_catalogs
         SET title = ?2,
             source_url = ?3,
             source_hash = ?4,
             refreshed_at = ?5
         WHERE catalog_key = ?1",
    )
    .bind(&catalog.catalog_key)
    .bind(&catalog.title)
    .bind(&catalog.source_url)
    .bind(&catalog.source_hash)
    .bind(refreshed_at)
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to refresh Yuc catalog timestamp"))?;
    Ok(())
}

async fn store_catalog(pool: &SqlitePool, catalog: &YucCatalog, now: &str) -> Result<(), AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start Yuc catalog transaction"))?;

    sqlx::query(
        "INSERT INTO yuc_catalogs (
            catalog_key,
            catalog_kind,
            season_year,
            season_month,
            title,
            source_url,
            source_hash,
            fetched_at,
            refreshed_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
         ON CONFLICT(catalog_key) DO UPDATE SET
            catalog_kind = excluded.catalog_kind,
            season_year = excluded.season_year,
            season_month = excluded.season_month,
            title = excluded.title,
            source_url = excluded.source_url,
            source_hash = excluded.source_hash,
            fetched_at = excluded.fetched_at,
            refreshed_at = excluded.refreshed_at",
    )
    .bind(&catalog.catalog_key)
    .bind(&catalog.catalog_kind)
    .bind(catalog.season_year)
    .bind(i64::from(catalog.season_month))
    .bind(&catalog.title)
    .bind(&catalog.source_url)
    .bind(&catalog.source_hash)
    .bind(now)
    .execute(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to upsert Yuc catalog"))?;

    let catalog_id =
        sqlx::query_scalar::<_, i64>("SELECT id FROM yuc_catalogs WHERE catalog_key = ?1 LIMIT 1")
            .bind(&catalog.catalog_key)
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| AppError::internal("failed to load Yuc catalog id"))?;

    sqlx::query("DELETE FROM yuc_catalog_entries WHERE yuc_catalog_id = ?1")
        .bind(catalog_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::internal("failed to clear stale Yuc catalog entries"))?;

    for entry in &catalog.entries {
        sqlx::query(
            "INSERT INTO yuc_catalog_entries (
                yuc_catalog_id,
                sort_index,
                weekday_id,
                weekday_cn,
                weekday_en,
                weekday_ja,
                title,
                title_cn,
                title_original,
                broadcast_time,
                broadcast_label,
                episode_note,
                platform_note,
                source_image_url,
                created_at,
                updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)",
        )
        .bind(catalog_id)
        .bind(entry.sort_index)
        .bind(i64::from(entry.weekday.id))
        .bind(&entry.weekday.cn)
        .bind(&entry.weekday.en)
        .bind(&entry.weekday.ja)
        .bind(&entry.title)
        .bind(&entry.title_cn)
        .bind(entry.title_original.as_deref())
        .bind(entry.broadcast_time.as_deref())
        .bind(entry.broadcast_label.as_deref())
        .bind(entry.episode_note.as_deref())
        .bind(entry.platform_note.as_deref())
        .bind(entry.source_image_url.as_deref())
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::internal("failed to insert Yuc catalog entry"))?;
    }

    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit Yuc catalog transaction"))?;

    Ok(())
}

async fn populate_missing_matches(
    pool: &SqlitePool,
    bangumi: &BangumiClient,
    catalog_key: &str,
) -> Result<(), AppError> {
    let entries = sqlx::query_as::<_, CatalogMatchRow>(
        "SELECT
            yuc_catalog_entries.id,
            yuc_catalog_entries.title,
            yuc_catalog_entries.title_cn,
            yuc_catalog_entries.title_original
         FROM yuc_catalog_entries
         INNER JOIN yuc_catalogs ON yuc_catalogs.id = yuc_catalog_entries.yuc_catalog_id
         WHERE yuc_catalogs.catalog_key = ?1
           AND yuc_catalog_entries.bangumi_matched_at IS NULL
         ORDER BY yuc_catalog_entries.sort_index ASC",
    )
    .bind(catalog_key)
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::internal("failed to list Yuc entries needing Bangumi match"))?;

    if entries.is_empty() {
        return Ok(());
    }

    let matched_at = Utc::now().to_rfc3339();
    let resolutions = stream::iter(entries.into_iter().map(|entry| {
        let bangumi = bangumi.clone();
        async move {
            let resolution = resolve_bangumi_match(&bangumi, &entry).await;
            (entry.id, resolution)
        }
    }))
    .buffer_unordered(MATCH_CONCURRENCY)
    .collect::<Vec<_>>()
    .await;

    for (entry_id, resolution) in resolutions {
        if let Some(card) = resolution.card.as_ref() {
            upsert_subject_cache(pool, card, &matched_at).await?;
        }

        sqlx::query(
            "UPDATE yuc_catalog_entries
             SET bangumi_subject_id = ?2,
                 bangumi_match_score = ?3,
                 bangumi_match_title = ?4,
                 bangumi_matched_at = ?5,
                 updated_at = ?5
             WHERE id = ?1",
        )
        .bind(entry_id)
        .bind(resolution.subject_id)
        .bind(resolution.score)
        .bind(resolution.matched_title.as_deref())
        .bind(&matched_at)
        .execute(pool)
        .await
        .map_err(|_| AppError::internal("failed to store Yuc Bangumi match result"))?;
    }

    Ok(())
}

async fn refresh_subject_statuses(
    pool: &SqlitePool,
    bangumi: &BangumiClient,
    catalog_key: &str,
) -> Result<(), AppError> {
    let subject_ids = sqlx::query_scalar::<_, i64>(
        "SELECT DISTINCT yuc_catalog_entries.bangumi_subject_id
         FROM yuc_catalog_entries
         INNER JOIN yuc_catalogs ON yuc_catalogs.id = yuc_catalog_entries.yuc_catalog_id
         WHERE yuc_catalogs.catalog_key = ?1
           AND yuc_catalog_entries.bangumi_subject_id IS NOT NULL",
    )
    .bind(catalog_key)
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::internal("failed to list Bangumi status refresh candidates"))?;

    if subject_ids.is_empty() {
        return Ok(());
    }

    let refreshed_at = Utc::now().to_rfc3339();
    let cards = stream::iter(subject_ids.into_iter().map(|subject_id| {
        let bangumi = bangumi.clone();
        async move {
            match bangumi.fetch_subject(subject_id).await {
                Ok(subject) => {
                    let episodes = match bangumi.fetch_episodes(subject_id).await {
                        Ok(episodes) => episodes,
                        Err(error) => {
                            warn!(
                                subject_id,
                                error = %error,
                                "Failed to refresh Bangumi episode state for Yuc catalog"
                            );
                            Vec::new()
                        }
                    };

                    let mut card = subject.to_card();
                    card.release_status = derive_release_status(&subject, &episodes).to_owned();
                    Some(card)
                }
                Err(error) => {
                    warn!(
                        subject_id,
                        error = %error,
                        "Failed to refresh Bangumi subject status for Yuc catalog"
                    );
                    None
                }
            }
        }
    }))
    .buffer_unordered(STATUS_REFRESH_CONCURRENCY)
    .filter_map(|item| async move { item })
    .collect::<Vec<_>>()
    .await;

    for card in cards {
        upsert_subject_cache(pool, &card, &refreshed_at).await?;
    }

    Ok(())
}

async fn upsert_subject_cache(
    pool: &SqlitePool,
    card: &SubjectCardDto,
    refreshed_at: &str,
) -> Result<(), AppError> {
    let tags_json = serde_json::to_string(&card.tags)
        .map_err(|_| AppError::internal("failed to serialize Bangumi cache tags"))?;

    sqlx::query(
        "INSERT INTO bangumi_subject_cache (
            bangumi_subject_id,
            title,
            title_cn,
            summary,
            air_date,
            air_weekday,
            total_episodes,
            image_portrait,
            image_banner,
            tags_json,
            rating_score,
            release_status,
            metadata_refreshed_at,
            status_refreshed_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)
         ON CONFLICT(bangumi_subject_id) DO UPDATE SET
            title = excluded.title,
            title_cn = excluded.title_cn,
            summary = excluded.summary,
            air_date = excluded.air_date,
            air_weekday = excluded.air_weekday,
            total_episodes = excluded.total_episodes,
            image_portrait = excluded.image_portrait,
            image_banner = excluded.image_banner,
            tags_json = excluded.tags_json,
            rating_score = excluded.rating_score,
            release_status = excluded.release_status,
            metadata_refreshed_at = excluded.metadata_refreshed_at,
            status_refreshed_at = excluded.status_refreshed_at",
    )
    .bind(card.bangumi_subject_id)
    .bind(&card.title)
    .bind(&card.title_cn)
    .bind(&card.summary)
    .bind(card.air_date.as_deref())
    .bind(card.air_weekday.map(i64::from))
    .bind(card.total_episodes)
    .bind(card.image_portrait.as_deref())
    .bind(card.image_banner.as_deref())
    .bind(tags_json)
    .bind(card.rating_score)
    .bind(&card.release_status)
    .bind(refreshed_at)
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to upsert Bangumi subject cache"))?;

    Ok(())
}

async fn load_calendar_rows(
    pool: &SqlitePool,
    catalog_key: &str,
) -> Result<Vec<CalendarEntryRow>, AppError> {
    sqlx::query_as::<_, CalendarEntryRow>(
        "SELECT
            yuc_catalog_entries.weekday_id,
            yuc_catalog_entries.title,
            yuc_catalog_entries.title_cn,
            yuc_catalog_entries.title_original,
            yuc_catalog_entries.broadcast_time,
            yuc_catalog_entries.bangumi_subject_id,
            yuc_catalog_entries.source_image_url,
            bangumi_subject_cache.title AS cached_title,
            bangumi_subject_cache.title_cn AS cached_title_cn,
            bangumi_subject_cache.summary,
            bangumi_subject_cache.air_date,
            bangumi_subject_cache.air_weekday,
            bangumi_subject_cache.image_portrait,
            bangumi_subject_cache.image_banner,
            bangumi_subject_cache.tags_json,
            bangumi_subject_cache.total_episodes,
            bangumi_subject_cache.rating_score,
            bangumi_subject_cache.release_status
         FROM yuc_catalog_entries
         INNER JOIN yuc_catalogs ON yuc_catalogs.id = yuc_catalog_entries.yuc_catalog_id
         LEFT JOIN bangumi_subject_cache
            ON bangumi_subject_cache.bangumi_subject_id = yuc_catalog_entries.bangumi_subject_id
         WHERE yuc_catalogs.catalog_key = ?1
         ORDER BY yuc_catalog_entries.weekday_id ASC, yuc_catalog_entries.sort_index ASC",
    )
    .bind(catalog_key)
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::internal("failed to load Yuc calendar rows"))
}

fn parse_weekday_entries(html: &str) -> Vec<YucCatalogEntry> {
    let mut entries = Vec::new();
    let mut sort_index = 0i64;

    for capture in weekday_block_regex().captures_iter(html) {
        let Some(marker) = capture.name("weekday").map(|value| value.as_str().trim()) else {
            continue;
        };
        let Some(content) = capture.name("content").map(|value| value.as_str()) else {
            continue;
        };
        let Some(weekday) = weekday_from_marker(marker) else {
            continue;
        };

        for card in weekday_card_regex().captures_iter(content) {
            let Some(raw_title) = card.name("title").map(|value| value.as_str()) else {
                continue;
            };

            let title_cn = sanitize_title(raw_title);
            if title_cn.is_empty() {
                continue;
            }

            sort_index += 1;
            entries.push(YucCatalogEntry {
                sort_index,
                weekday: weekday.clone(),
                title: title_cn.clone(),
                title_cn,
                title_original: None,
                broadcast_time: card
                    .name("time")
                    .map(|value| value.as_str().trim().to_owned())
                    .filter(|value| !value.is_empty()),
                broadcast_label: None,
                episode_note: card
                    .name("episode_note")
                    .map(|value| sanitize_title(value.as_str()))
                    .filter(|value| !value.is_empty()),
                platform_note: card
                    .name("area")
                    .map(|value| parse_platform_note(value.as_str()))
                    .filter(|value| !value.is_empty()),
                source_image_url: card
                    .name("image")
                    .map(|value| value.as_str().trim().to_owned())
                    .filter(|value| !value.is_empty()),
            });
        }
    }

    entries
}

fn parse_detail_entries(html: &str) -> Vec<DetailScheduleEntry> {
    detail_card_regex()
        .captures_iter(html)
        .filter_map(|capture| {
            let title_cn = capture
                .name("title_cn")
                .map(|value| sanitize_title(value.as_str()))
                .unwrap_or_default();
            let title_original = capture
                .name("title_original")
                .map(|value| sanitize_title(value.as_str()))
                .unwrap_or_default();
            if title_cn.is_empty() {
                return None;
            }

            Some(DetailScheduleEntry {
                normalized_cn: normalize_title(&title_cn),
                stripped_cn: strip_variant(&title_cn),
                title_original,
                broadcast_label: capture
                    .name("broadcast")
                    .map(|value| sanitize_title(value.as_str()))
                    .filter(|value| !value.is_empty()),
            })
        })
        .collect()
}

fn attach_detail_entries(entries: &mut [YucCatalogEntry], details: &[DetailScheduleEntry]) {
    for entry in entries {
        let Some((detail, score)) = details
            .iter()
            .map(|detail| {
                (
                    detail,
                    score_text_pair(
                        &normalize_title(&entry.title_cn),
                        &strip_variant(&entry.title_cn),
                        &detail.normalized_cn,
                        &detail.stripped_cn,
                    ),
                )
            })
            .max_by_key(|(_, score)| *score)
        else {
            continue;
        };

        if score < 72 {
            continue;
        }

        if !detail.title_original.is_empty() {
            entry.title_original = Some(detail.title_original.clone());
        }

        if entry.broadcast_label.is_none() {
            entry.broadcast_label = detail.broadcast_label.clone();
        }
    }
}

async fn resolve_bangumi_match(
    bangumi: &BangumiClient,
    entry: &CatalogMatchRow,
) -> BangumiMatchResolution {
    let search_terms = build_search_terms(entry);
    let mut candidates = HashMap::<i64, (f64, SubjectRaw)>::new();

    for term in search_terms {
        let query = BangumiSearchQuery {
            keyword: term.clone(),
            sort: "match".to_owned(),
            tags: Vec::new(),
            meta_tags: Vec::new(),
            air_date_start: None,
            air_date_end: None,
            rating_min: None,
            rating_max: None,
            rating_count_min: None,
            rating_count_max: None,
            rank_min: None,
            rank_max: None,
            nsfw: None,
        };

        let response = match bangumi.search_subjects(&query, 8, 0).await {
            Ok(response) => response,
            Err(error) => {
                warn!(
                    entry_id = entry.id,
                    keyword = %term,
                    error = %error,
                    "Failed to search Bangumi while resolving Yuc catalog entry"
                );
                continue;
            }
        };

        for subject in response.data {
            let score = score_subject_candidate(&subject, entry);
            let existing = candidates.get(&subject.id).map(|(value, _)| *value);
            if existing.is_none_or(|value| score > value) {
                candidates.insert(subject.id, (score, subject));
            }
        }
    }

    let mut scored = candidates.into_values().collect::<Vec<_>>();
    scored.sort_by(|left, right| right.0.total_cmp(&left.0));

    let Some((best_score, best_subject)) = scored.into_iter().next() else {
        return BangumiMatchResolution {
            subject_id: None,
            score: None,
            matched_title: None,
            card: None,
        };
    };

    if best_score < 68.0 {
        return BangumiMatchResolution {
            subject_id: None,
            score: Some(best_score),
            matched_title: None,
            card: None,
        };
    }

    BangumiMatchResolution {
        subject_id: Some(best_subject.id),
        score: Some(best_score),
        matched_title: Some(preferred_subject_title(&best_subject)),
        card: Some(best_subject.to_card()),
    }
}

fn build_search_terms(entry: &CatalogMatchRow) -> Vec<String> {
    let mut terms = Vec::new();
    for candidate in [
        entry.title_original.as_deref(),
        Some(entry.title.as_str()),
        Some(entry.title_cn.as_str()),
    ] {
        let Some(value) = candidate.map(str::trim).filter(|value| !value.is_empty()) else {
            continue;
        };

        if terms.iter().any(|existing| existing == value) {
            continue;
        }

        terms.push(value.to_owned());
    }

    terms
}

fn score_subject_candidate(subject: &SubjectRaw, entry: &CatalogMatchRow) -> f64 {
    let left = [subject.name.as_str(), subject.name_cn.as_str()];
    let right = [
        entry.title_original.as_deref().unwrap_or_default(),
        entry.title.as_str(),
        entry.title_cn.as_str(),
    ];

    let mut best: f64 = 0.0;
    for left_value in left {
        for right_value in right {
            if left_value.trim().is_empty() || right_value.trim().is_empty() {
                continue;
            }

            best = best.max(f64::from(score_text_pair(
                &normalize_title(left_value),
                &strip_variant(left_value),
                &normalize_title(right_value),
                &strip_variant(right_value),
            )));
        }
    }

    best
}

fn preferred_subject_title(subject: &SubjectRaw) -> String {
    if !subject.name_cn.trim().is_empty() {
        subject.name_cn.clone()
    } else {
        subject.name.clone()
    }
}

fn ordered_weekdays() -> Vec<WeekdayDto> {
    vec![
        WeekdayDto {
            id: 1,
            cn: "星期一".to_owned(),
            en: "Mon".to_owned(),
            ja: "月曜".to_owned(),
        },
        WeekdayDto {
            id: 2,
            cn: "星期二".to_owned(),
            en: "Tue".to_owned(),
            ja: "火曜".to_owned(),
        },
        WeekdayDto {
            id: 3,
            cn: "星期三".to_owned(),
            en: "Wed".to_owned(),
            ja: "水曜".to_owned(),
        },
        WeekdayDto {
            id: 4,
            cn: "星期四".to_owned(),
            en: "Thu".to_owned(),
            ja: "木曜".to_owned(),
        },
        WeekdayDto {
            id: 5,
            cn: "星期五".to_owned(),
            en: "Fri".to_owned(),
            ja: "金曜".to_owned(),
        },
        WeekdayDto {
            id: 6,
            cn: "星期六".to_owned(),
            en: "Sat".to_owned(),
            ja: "土曜".to_owned(),
        },
        WeekdayDto {
            id: 7,
            cn: "星期日".to_owned(),
            en: "Sun".to_owned(),
            ja: "日曜".to_owned(),
        },
    ]
}

fn weekday_from_marker(value: &str) -> Option<WeekdayDto> {
    let id = match value {
        "周一" => 1,
        "周二" => 2,
        "周三" => 3,
        "周四" => 4,
        "周五" => 5,
        "周六" => 6,
        "周日" => 7,
        _ => return None,
    };

    ordered_weekdays()
        .into_iter()
        .find(|weekday| weekday.id == id)
}

fn parse_platform_note(raw: &str) -> String {
    let mut values = area_regex()
        .captures_iter(raw)
        .filter_map(|capture| {
            capture
                .name("name")
                .map(|value| sanitize_title(value.as_str()))
        })
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values.join(" / ")
}

fn parse_tags_json(raw: &str) -> Option<Vec<String>> {
    let parsed = serde_json::from_str::<Vec<String>>(raw).ok()?;
    Some(parsed.into_iter().take(8).collect())
}

fn parse_catalog_key(value: &str) -> Option<(i32, u32)> {
    if value.len() != 6 {
        return None;
    }

    let year = value.get(0..4)?.parse::<i32>().ok()?;
    let month = value.get(4..6)?.parse::<u32>().ok()?;
    Some((year, month))
}

fn is_stale_rfc3339(value: &str, threshold_hours: i64) -> bool {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|parsed| parsed.with_timezone(&Utc))
        .map(|parsed| Utc::now() - parsed > chrono::Duration::hours(threshold_hours))
        .unwrap_or(true)
}

fn hash_text(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn sanitize_title(raw: &str) -> String {
    let without_tags = html_tag_regex().replace_all(raw, " ");
    without_tags
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&ldquo;", "\"")
        .replace("&rdquo;", "\"")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn strip_variant(value: &str) -> String {
    let stripped = variant_regex().replace_all(value, "");
    normalize_title(&stripped)
}

fn normalize_title(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn score_text_pair(
    left_normalized: &str,
    left_stripped: &str,
    right_normalized: &str,
    right_stripped: &str,
) -> i32 {
    let mut score = 0;

    if !left_normalized.is_empty() && left_normalized == right_normalized {
        score = score.max(140);
    }

    if !left_stripped.is_empty() && left_stripped == right_stripped {
        score = score.max(136);
    }

    if !left_stripped.is_empty()
        && !right_stripped.is_empty()
        && (left_normalized.contains(right_stripped)
            || right_normalized.contains(left_stripped)
            || left_stripped.contains(right_stripped)
            || right_stripped.contains(left_stripped))
    {
        score = score.max(108);
    }

    score = score.max((dice_coefficient(left_normalized, right_normalized) * 100.0).round() as i32);
    score = score.max((dice_coefficient(left_stripped, right_stripped) * 112.0).round() as i32);

    score
}

fn dice_coefficient(left: &str, right: &str) -> f32 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    if left == right {
        return 1.0;
    }

    let left_pairs = bigrams(left);
    let right_pairs = bigrams(right);
    if left_pairs.is_empty() || right_pairs.is_empty() {
        return 0.0;
    }

    let mut overlap = 0usize;
    let mut counts = HashMap::new();
    for pair in &left_pairs {
        *counts.entry(pair.clone()).or_insert(0usize) += 1;
    }

    for pair in &right_pairs {
        if let Some(count) = counts.get_mut(pair) {
            if *count > 0 {
                *count -= 1;
                overlap += 1;
            }
        }
    }

    (2 * overlap) as f32 / (left_pairs.len() + right_pairs.len()) as f32
}

fn bigrams(value: &str) -> Vec<String> {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() < 2 {
        return Vec::new();
    }

    chars
        .windows(2)
        .map(|window| window.iter().collect::<String>())
        .collect()
}

fn sort_cards_by_broadcast_time(items: &mut [SubjectCardDto]) {
    items.sort_by(|left, right| {
        match (
            parse_broadcast_time(left.broadcast_time.as_deref()),
            parse_broadcast_time(right.broadcast_time.as_deref()),
        ) {
            (Some(left_key), Some(right_key)) => left_key.cmp(&right_key),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left
                .title_cn
                .cmp(&right.title_cn)
                .then_with(|| left.title.cmp(&right.title)),
        }
    });
}

fn parse_broadcast_time(value: Option<&str>) -> Option<u16> {
    let value = value?.trim();
    let (hour, minute) = value.split_once(':')?;
    let hour = hour.parse::<u16>().ok()?;
    let minute = minute.parse::<u16>().ok()?;
    Some(hour * 60 + minute)
}

fn derive_release_status(subject: &SubjectRaw, episodes: &[EpisodeRaw]) -> &'static str {
    let today = Local::now().date_naive();

    if parse_subject_date(subject.air_date.as_ref().or(subject.date.as_ref()))
        .is_some_and(|date| date > today)
    {
        return "upcoming";
    }

    let episode_dates = episodes
        .iter()
        .filter_map(|episode| parse_episode_airdate(&episode.airdate))
        .collect::<Vec<_>>();

    if episode_dates.is_empty() {
        return fallback_release_status(subject, today);
    }

    let aired_count = episode_dates.iter().filter(|date| **date <= today).count();
    let future_count = episode_dates.len().saturating_sub(aired_count);

    if aired_count == 0 && future_count > 0 {
        return "upcoming";
    }

    if future_count > 0 {
        return "airing";
    }

    if subject
        .total_episodes
        .and_then(|total| usize::try_from(total).ok())
        .is_some_and(|total| aired_count < total)
    {
        return "airing";
    }

    "completed"
}

fn fallback_release_status(subject: &SubjectRaw, today: NaiveDate) -> &'static str {
    if parse_subject_date(subject.air_date.as_ref().or(subject.date.as_ref()))
        .is_some_and(|date| date > today)
    {
        return "upcoming";
    }

    let mut markers = String::new();
    for item in &subject.infobox {
        markers.push_str(&item.key);
        markers.push(' ');
        markers.push_str(&flatten_infobox_value(&item.value));
        markers.push(' ');
    }

    let markers = markers.to_lowercase();
    if markers.contains("放送中")
        || markers.contains("播出中")
        || markers.contains("播放中")
        || markers.contains("连载中")
        || markers.contains("連載中")
        || markers.contains("配信中")
        || markers.contains("上映中")
        || markers.contains("airing")
        || markers.contains("ongoing")
    {
        return "airing";
    }

    "completed"
}

fn parse_subject_date(value: Option<&String>) -> Option<NaiveDate> {
    let value = value?;
    parse_episode_airdate(value)
}

fn parse_episode_airdate(value: &str) -> Option<NaiveDate> {
    let date_part = value
        .trim()
        .split_once('T')
        .map(|(left, _)| left)
        .unwrap_or(value)
        .trim();

    if date_part.is_empty() {
        return None;
    }

    NaiveDate::parse_from_str(date_part, "%Y-%m-%d").ok()
}

fn flatten_infobox_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Number(number) => number.to_string(),
        serde_json::Value::Bool(boolean) => boolean.to_string(),
        serde_json::Value::Array(items) => items
            .iter()
            .map(flatten_infobox_value)
            .filter(|item| !item.is_empty())
            .collect::<Vec<_>>()
            .join(" / "),
        serde_json::Value::Object(map) => map
            .get("v")
            .map(flatten_infobox_value)
            .or_else(|| map.get("name").map(flatten_infobox_value))
            .unwrap_or_default(),
    }
}

fn weekday_block_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"(?s)<!--(?P<weekday>周[一二三四五六日])-->\s*<div><table class="date_"[^>]*><tr><td class="date2">.*?</td></tr></table></div>\s*<div>(?P<content>.*?)</div><div style="clear:both"></div>"#,
        )
        .expect("valid yuc weekday block regex")
    })
}

fn weekday_card_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"(?s)<div style="float:left"><div class="div_date"><p class="imgtext\d+">(?P<time>\d{2}:\d{2})~</p><p class="imgep">(?P<episode_note>.*?)</p><img[^>]*data-src="(?P<image>[^"]+)"[^>]*></div><div><table width="120px"><tr><td colspan="3" class="date_title_[^"]*">(?P<title>.*?)</td></tr><tr class="tr_area">(?P<area>.*?)</tr></table></div></div>"#,
        )
        .expect("valid yuc weekday card regex")
    })
}

fn detail_card_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"(?s)<p class="title_cn_r\d*">(?P<title_cn>.*?)</p>\s*<p class="title_jp_r\d*">(?P<title_original>.*?)</p>.*?<p class="broadcast_r">(?P<broadcast>.*?)</p>"#,
        )
        .expect("valid yuc detail regex")
    })
}

fn area_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(r#"(?s)<p class="area">(?P<name>.*?)</p>"#).expect("valid yuc area regex")
    })
}

fn html_tag_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"<[^>]+>").expect("valid html tag regex"))
}

fn variant_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r"(第\s*[0-9一二三四五六七八九十百零两]+\s*(?:季|期|部|章|篇|弹|作)|[Pp]art\.?\s*[0-9]+|[Ss]eason\s*[0-9]+|最终章|最终季|完结篇)",
        )
        .expect("valid title variant regex")
    })
}
