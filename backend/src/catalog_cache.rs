use std::{
    collections::{HashMap, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
};

use chrono::Utc;
use futures::stream::{self, StreamExt};
use sqlx::{FromRow, SqlitePool};
use tracing::warn;

use crate::{
    bangumi::{BangumiClient, BangumiSearchQuery, SubjectRaw},
    season_catalog::derive_release_status,
    types::{
        AppError, CatalogManifestResponse, CatalogPageResponse, CatalogSectionDto, SubjectCardDto,
    },
    yuc::YucClient,
};

const CATALOG_REFRESH_TTL_HOURS: i64 = 12;
const MATCH_CONCURRENCY: usize = 6;
const STATUS_REFRESH_CONCURRENCY: usize = 6;
const INITIAL_STATUS_REFRESH_AT: &str = "1970-01-01T00:00:00Z";

#[derive(Debug, Clone, Copy)]
enum CatalogKind {
    Preview,
    Special,
}

impl CatalogKind {
    fn from_path(kind: &str) -> Option<Self> {
        match kind.trim().to_ascii_lowercase().as_str() {
            "preview" => Some(Self::Preview),
            "special" => Some(Self::Special),
            _ => None,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Preview => "preview",
            Self::Special => "special",
        }
    }

    fn source_url(self, yuc: &YucClient) -> String {
        match self {
            Self::Preview => yuc.preview_url(),
            Self::Special => yuc.special_url(),
        }
    }
}

#[derive(Debug, Clone)]
struct CatalogSyncPayload {
    key: String,
    kind: String,
    title: String,
    source_url: String,
    source_hash: String,
    entries: Vec<CatalogEntrySeed>,
}

#[derive(Debug, Clone)]
struct CatalogEntrySeed {
    sort_index: i64,
    group_key: String,
    group_title: String,
    title: String,
    title_cn: String,
    title_original: Option<String>,
    source_image_url: Option<String>,
    catalog_label: Option<String>,
    entry_release_status: String,
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
struct StatusRefreshCandidateRow {
    bangumi_subject_id: i64,
    status_refreshed_at: Option<String>,
}

#[derive(Debug, Clone, FromRow)]
struct CatalogPageRow {
    entry_id: i64,
    group_key: Option<String>,
    group_title: Option<String>,
    title: String,
    title_cn: String,
    title_original: Option<String>,
    source_image_url: Option<String>,
    catalog_label: Option<String>,
    entry_release_status: Option<String>,
    bangumi_subject_id: Option<i64>,
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

impl CatalogPageRow {
    fn section_key(&self) -> String {
        self.group_key
            .clone()
            .unwrap_or_else(|| format!("catalog-{}", self.entry_id))
    }

    fn section_title(&self) -> String {
        self.group_title
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| "目录".to_owned())
    }

    fn to_card(&self) -> SubjectCardDto {
        let bangumi_subject_id = self.bangumi_subject_id.unwrap_or(-self.entry_id);
        let matched = bangumi_subject_id > 0;
        SubjectCardDto {
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
                .entry_release_status
                .clone()
                .or_else(|| self.release_status.clone())
                .unwrap_or_else(|| "completed".to_owned()),
            air_date: self.air_date.clone(),
            broadcast_time: None,
            air_weekday: self.air_weekday.and_then(|value| u8::try_from(value).ok()),
            image_portrait: self
                .image_portrait
                .clone()
                .or_else(|| self.source_image_url.clone()),
            image_banner: self
                .image_banner
                .clone()
                .or_else(|| self.source_image_url.clone()),
            tags: if matched {
                self.tags_json
                    .as_deref()
                    .and_then(parse_tags_json)
                    .unwrap_or_default()
            } else {
                Vec::new()
            },
            total_episodes: self.total_episodes,
            rating_score: if matched { self.rating_score } else { None },
            catalog_label: self.catalog_label.clone(),
        }
    }
}

pub async fn load_catalog_manifest(
    yuc: &YucClient,
    pool: &SqlitePool,
    bangumi: &BangumiClient,
) -> Result<CatalogManifestResponse, AppError> {
    let preview_sync = sync_catalog(yuc, pool, bangumi, CatalogKind::Preview).await;
    if let Err(error) = preview_sync.as_ref() {
        warn!(error = %error, "Failed to refresh preview catalog; attempting cached fallback");
    }

    let special_sync = sync_catalog(yuc, pool, bangumi, CatalogKind::Special).await;
    if let Err(error) = special_sync.as_ref() {
        warn!(error = %error, "Failed to refresh special catalog; attempting cached fallback");
    }

    let preview_available = has_catalog_entries(pool, CatalogKind::Preview.key()).await?;
    let special_available = has_catalog_entries(pool, CatalogKind::Special.key()).await?;

    if !preview_available {
        if let Err(error) = preview_sync {
            return Err(error);
        }
    }

    if !special_available {
        if let Err(error) = special_sync {
            return Err(error);
        }
    }

    Ok(CatalogManifestResponse {
        preview_available,
        special_available,
    })
}

pub async fn load_catalog_page(
    yuc: &YucClient,
    pool: &SqlitePool,
    bangumi: &BangumiClient,
    kind: &str,
) -> Result<CatalogPageResponse, AppError> {
    let kind = CatalogKind::from_path(kind)
        .ok_or_else(|| AppError::not_found("unknown Yuc catalog page"))?;

    let sync_result = sync_catalog(yuc, pool, bangumi, kind).await;
    if let Err(error) = sync_result.as_ref() {
        warn!(
            catalog_key = kind.key(),
            error = %error,
            "Failed to refresh Yuc catalog page; attempting cached fallback"
        );
    }

    let title = load_catalog_title(pool, kind.key()).await?;
    let rows = load_catalog_rows(pool, kind.key()).await?;
    if rows.is_empty() {
        if let Err(error) = sync_result {
            return Err(error);
        }

        return Err(AppError::internal("catalog page is empty"));
    }

    let sections = rows_into_sections(rows);

    Ok(CatalogPageResponse {
        kind: kind.key().to_owned(),
        title,
        sections,
    })
}

async fn sync_catalog(
    yuc: &YucClient,
    pool: &SqlitePool,
    bangumi: &BangumiClient,
    kind: CatalogKind,
) -> Result<(), AppError> {
    let snapshot = load_catalog_snapshot(pool, kind.key()).await?;
    let needs_fetch = snapshot
        .as_ref()
        .is_none_or(|row| is_stale_rfc3339(&row.refreshed_at, CATALOG_REFRESH_TTL_HOURS))
        || catalog_requires_shape_refresh(pool, kind).await?;

    if needs_fetch {
        let payload = fetch_catalog_payload(yuc, kind).await?;
        if snapshot
            .as_ref()
            .is_some_and(|row| row.source_hash == payload.source_hash)
        {
            touch_catalog_refresh(pool, &payload).await?;
        } else {
            store_catalog(pool, &payload).await?;
        }
    }

    populate_missing_matches(pool, bangumi, kind.key()).await?;
    refresh_subject_statuses(pool, bangumi, kind.key()).await?;
    Ok(())
}

async fn catalog_requires_shape_refresh(
    pool: &SqlitePool,
    kind: CatalogKind,
) -> Result<bool, AppError> {
    if !matches!(kind, CatalogKind::Preview) {
        return Ok(false);
    }

    let title = sqlx::query_scalar::<_, String>(
        "SELECT title
         FROM yuc_catalogs
         WHERE catalog_key = ?1
         LIMIT 1",
    )
    .bind(kind.key())
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::internal("failed to inspect cached catalog shape"))?;

    Ok(title
        .as_deref()
        .is_some_and(|value| value.trim() != "新季度前瞻"))
}

async fn fetch_catalog_payload(
    yuc: &YucClient,
    kind: CatalogKind,
) -> Result<CatalogSyncPayload, AppError> {
    let (title, sections) = match kind {
        CatalogKind::Preview => yuc.fetch_preview_catalog().await?,
        CatalogKind::Special => yuc.fetch_special_catalog().await?,
    };

    let entries = sections
        .iter()
        .enumerate()
        .flat_map(|(section_index, section)| {
            section
                .items
                .iter()
                .enumerate()
                .map(move |(item_index, item)| CatalogEntrySeed {
                    sort_index: ((section_index as i64) + 1) * 10_000 + item_index as i64,
                    group_key: section.key.clone(),
                    group_title: section.title.clone(),
                    title: item.title.clone(),
                    title_cn: item.title_cn.clone(),
                    title_original: if item.title != item.title_cn {
                        Some(item.title.clone())
                    } else {
                        None
                    },
                    source_image_url: normalize_image_url(item.image_portrait.as_deref()),
                    catalog_label: item.catalog_label.clone(),
                    entry_release_status: item.release_status.clone(),
                })
        })
        .collect::<Vec<_>>();

    let source_hash = hash_catalog_payload(&title, &entries)?;

    Ok(CatalogSyncPayload {
        key: kind.key().to_owned(),
        kind: kind.key().to_owned(),
        title,
        source_url: kind.source_url(yuc),
        source_hash,
        entries,
    })
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
    .map_err(|_| AppError::internal("failed to load catalog snapshot"))
}

async fn touch_catalog_refresh(
    pool: &SqlitePool,
    payload: &CatalogSyncPayload,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE yuc_catalogs
         SET title = ?2,
             source_url = ?3,
             source_hash = ?4,
             refreshed_at = ?5
         WHERE catalog_key = ?1",
    )
    .bind(&payload.key)
    .bind(&payload.title)
    .bind(&payload.source_url)
    .bind(&payload.source_hash)
    .bind(now_string())
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to refresh catalog metadata"))?;

    Ok(())
}

async fn store_catalog(pool: &SqlitePool, payload: &CatalogSyncPayload) -> Result<(), AppError> {
    let now = now_string();
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start catalog transaction"))?;

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
         ) VALUES (?1, ?2, NULL, NULL, ?3, ?4, ?5, ?6, ?6)
         ON CONFLICT(catalog_key) DO UPDATE SET
            catalog_kind = excluded.catalog_kind,
            title = excluded.title,
            source_url = excluded.source_url,
            source_hash = excluded.source_hash,
            fetched_at = excluded.fetched_at,
            refreshed_at = excluded.refreshed_at",
    )
    .bind(&payload.key)
    .bind(&payload.kind)
    .bind(&payload.title)
    .bind(&payload.source_url)
    .bind(&payload.source_hash)
    .bind(&now)
    .execute(&mut *tx)
    .await
    .map_err(|_| AppError::internal("failed to upsert cached catalog"))?;

    let catalog_id =
        sqlx::query_scalar::<_, i64>("SELECT id FROM yuc_catalogs WHERE catalog_key = ?1 LIMIT 1")
            .bind(&payload.key)
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| AppError::internal("failed to load cached catalog id"))?;

    sqlx::query("DELETE FROM yuc_catalog_entries WHERE yuc_catalog_id = ?1")
        .bind(catalog_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::internal("failed to clear cached catalog entries"))?;

    for entry in &payload.entries {
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
                group_key,
                group_title,
                catalog_label,
                entry_release_status,
                created_at,
                updated_at
             ) VALUES (
                ?1, ?2, 0, '', '', '', ?3, ?4, ?5, NULL, NULL, NULL, NULL, ?6, ?7, ?8, ?9, ?10, ?11, ?11
             )",
        )
        .bind(catalog_id)
        .bind(entry.sort_index)
        .bind(&entry.title)
        .bind(&entry.title_cn)
        .bind(entry.title_original.as_deref())
        .bind(entry.source_image_url.as_deref())
        .bind(&entry.group_key)
        .bind(&entry.group_title)
        .bind(entry.catalog_label.as_deref())
        .bind(&entry.entry_release_status)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::internal("failed to insert cached catalog entry"))?;
    }

    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit cached catalog transaction"))?;

    Ok(())
}

async fn load_catalog_title(pool: &SqlitePool, catalog_key: &str) -> Result<String, AppError> {
    sqlx::query_scalar::<_, String>(
        "SELECT title
         FROM yuc_catalogs
         WHERE catalog_key = ?1
         LIMIT 1",
    )
    .bind(catalog_key)
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::internal("failed to load cached catalog title"))
}

async fn has_catalog_entries(pool: &SqlitePool, catalog_key: &str) -> Result<bool, AppError> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*)
         FROM yuc_catalog_entries
         INNER JOIN yuc_catalogs ON yuc_catalogs.id = yuc_catalog_entries.yuc_catalog_id
         WHERE yuc_catalogs.catalog_key = ?1",
    )
    .bind(catalog_key)
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::internal("failed to count cached catalog entries"))?;

    Ok(count > 0)
}

async fn load_catalog_rows(
    pool: &SqlitePool,
    catalog_key: &str,
) -> Result<Vec<CatalogPageRow>, AppError> {
    sqlx::query_as::<_, CatalogPageRow>(
        "SELECT
            yuc_catalog_entries.id AS entry_id,
            yuc_catalog_entries.group_key,
            yuc_catalog_entries.group_title,
            yuc_catalog_entries.title,
            yuc_catalog_entries.title_cn,
            yuc_catalog_entries.title_original,
            yuc_catalog_entries.source_image_url,
            yuc_catalog_entries.catalog_label,
            yuc_catalog_entries.entry_release_status,
            yuc_catalog_entries.bangumi_subject_id,
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
         ORDER BY yuc_catalog_entries.sort_index ASC, yuc_catalog_entries.id ASC",
    )
    .bind(catalog_key)
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::internal("failed to load cached catalog rows"))
}

fn rows_into_sections(rows: Vec<CatalogPageRow>) -> Vec<CatalogSectionDto> {
    let mut sections = Vec::<CatalogSectionDto>::new();
    for row in rows {
        let section_key = row.section_key();
        if sections
            .last()
            .is_none_or(|section| section.key != section_key)
        {
            sections.push(CatalogSectionDto {
                key: section_key.clone(),
                title: row.section_title(),
                items: Vec::new(),
            });
        }

        if let Some(section) = sections.last_mut() {
            section.items.push(row.to_card());
        }
    }

    sections
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
    .map_err(|_| AppError::internal("failed to list entries needing Bangumi match"))?;

    if entries.is_empty() {
        return Ok(());
    }

    let matched_at = now_string();
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
            upsert_subject_cache(pool, card, &matched_at, INITIAL_STATUS_REFRESH_AT).await?;
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
        .map_err(|_| AppError::internal("failed to store catalog Bangumi match result"))?;
    }

    Ok(())
}

async fn refresh_subject_statuses(
    pool: &SqlitePool,
    bangumi: &BangumiClient,
    catalog_key: &str,
) -> Result<(), AppError> {
    let subject_ids = sqlx::query_as::<_, StatusRefreshCandidateRow>(
        "SELECT DISTINCT
            yuc_catalog_entries.bangumi_subject_id,
            bangumi_subject_cache.status_refreshed_at
         FROM yuc_catalog_entries
         INNER JOIN yuc_catalogs ON yuc_catalogs.id = yuc_catalog_entries.yuc_catalog_id
         LEFT JOIN bangumi_subject_cache
            ON bangumi_subject_cache.bangumi_subject_id = yuc_catalog_entries.bangumi_subject_id
         WHERE yuc_catalogs.catalog_key = ?1
           AND yuc_catalog_entries.bangumi_subject_id IS NOT NULL",
    )
    .bind(catalog_key)
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::internal("failed to list catalog status refresh candidates"))?;

    let subject_ids = subject_ids
        .into_iter()
        .filter(|row| status_refresh_due(row.status_refreshed_at.as_deref()))
        .map(|row| row.bangumi_subject_id)
        .collect::<Vec<_>>();

    if subject_ids.is_empty() {
        return Ok(());
    }

    let refreshed_at = now_string();
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
                                "Failed to refresh Bangumi episode state for cached catalog"
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
                        "Failed to refresh Bangumi subject state for cached catalog"
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
        upsert_subject_cache(pool, &card, &refreshed_at, &refreshed_at).await?;
    }

    Ok(())
}

async fn upsert_subject_cache(
    pool: &SqlitePool,
    card: &SubjectCardDto,
    metadata_refreshed_at: &str,
    status_refreshed_at: &str,
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
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
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
    .bind(metadata_refreshed_at)
    .bind(status_refreshed_at)
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to upsert Bangumi subject cache"))?;

    Ok(())
}

#[derive(Debug, Clone)]
struct BangumiMatchResolution {
    subject_id: Option<i64>,
    score: Option<f64>,
    matched_title: Option<String>,
    card: Option<SubjectCardDto>,
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
                    "Failed to search Bangumi while resolving cached catalog entry"
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

fn normalize_title(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn strip_variant(value: &str) -> String {
    value
        .chars()
        .filter(|character| {
            !character.is_whitespace()
                && !matches!(character, '(' | ')' | '[' | ']' | '第' | '季' | '期' | '部')
        })
        .flat_map(char::to_lowercase)
        .collect()
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

fn hash_catalog_payload(title: &str, entries: &[CatalogEntrySeed]) -> Result<String, AppError> {
    let source = serde_json::to_string(&(
        title,
        entries
            .iter()
            .map(|entry| {
                (
                    &entry.group_key,
                    &entry.group_title,
                    &entry.title,
                    &entry.title_cn,
                    &entry.title_original,
                    &entry.source_image_url,
                    &entry.catalog_label,
                    &entry.entry_release_status,
                )
            })
            .collect::<Vec<_>>(),
    ))
    .map_err(|_| AppError::internal("failed to serialize catalog payload"))?;

    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    Ok(format!("{:016x}", hasher.finish()))
}

fn now_string() -> String {
    Utc::now().to_rfc3339()
}

fn is_stale_rfc3339(value: &str, threshold_hours: i64) -> bool {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|parsed| parsed.with_timezone(&Utc))
        .map(|parsed| Utc::now() - parsed > chrono::Duration::hours(threshold_hours))
        .unwrap_or(true)
}

fn status_refresh_due(value: Option<&str>) -> bool {
    let Some(value) = value else {
        return true;
    };

    let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(value) else {
        return true;
    };

    parsed.with_timezone(&Utc).date_naive() < Utc::now().date_naive()
}

fn parse_tags_json(raw: &str) -> Option<Vec<String>> {
    let parsed = serde_json::from_str::<Vec<String>>(raw).ok()?;
    Some(parsed.into_iter().take(8).collect())
}

fn normalize_image_url(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.is_empty() {
        return None;
    }

    if let Some(rest) = value.strip_prefix("http://") {
        return Some(format!("https://{rest}"));
    }

    Some(value.to_owned())
}
