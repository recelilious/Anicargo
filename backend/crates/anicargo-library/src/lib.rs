use anicargo_bangumi::{BangumiClient, BangumiError, Episode, Subject};
use anicargo_media::{scan_media, MediaConfig, MediaEntry, MediaError};
use anitomy::{Anitomy, ElementCategory, Elements};
use serde::Serialize;
use serde_json::Value;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use std::fmt;
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::info;

#[derive(Debug)]
pub enum LibraryError {
    Media(MediaError),
    Sql(sqlx::Error),
    Io(std::io::Error),
    InvalidPath(String),
    InvalidInput(String),
    Bangumi(BangumiError),
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LibraryError::Media(err) => write!(f, "media error: {}", err),
            LibraryError::Sql(err) => write!(f, "database error: {}", err),
            LibraryError::Io(err) => write!(f, "io error: {}", err),
            LibraryError::InvalidPath(message) => write!(f, "invalid path: {}", message),
            LibraryError::InvalidInput(message) => write!(f, "invalid input: {}", message),
            LibraryError::Bangumi(err) => write!(f, "bangumi error: {}", err),
        }
    }
}

impl std::error::Error for LibraryError {}

impl From<MediaError> for LibraryError {
    fn from(err: MediaError) -> Self {
        LibraryError::Media(err)
    }
}

impl From<sqlx::Error> for LibraryError {
    fn from(err: sqlx::Error) -> Self {
        LibraryError::Sql(err)
    }
}

impl From<std::io::Error> for LibraryError {
    fn from(err: std::io::Error) -> Self {
        LibraryError::Io(err)
    }
}

impl From<BangumiError> for LibraryError {
    fn from(err: BangumiError) -> Self {
        LibraryError::Bangumi(err)
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct IndexSummary {
    pub scanned: usize,
    pub upserted: usize,
    pub parsed: usize,
    pub skipped: usize,
    pub removed: usize,
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct AutoMatchSummary {
    pub scanned: usize,
    pub candidates: usize,
    pub matched: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct AutoMatchOptions {
    pub limit: u32,
    pub min_candidate_score: f32,
    pub min_confidence: f32,
}

impl Default for AutoMatchOptions {
    fn default() -> Self {
        Self {
            limit: 8,
            min_candidate_score: 0.5,
            min_confidence: 0.9,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MatchCandidate {
    pub subject_id: i64,
    pub confidence: f32,
    pub reason: String,
    pub name: String,
    pub name_cn: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct MediaMatch {
    pub media_id: String,
    pub subject_id: i64,
    pub episode_id: Option<i64>,
    pub method: String,
    pub confidence: Option<f32>,
    pub reason: Option<String>,
}

#[derive(Debug, Default, Clone, Copy, Serialize)]
pub struct BangumiSyncSummary {
    pub subject_id: i64,
    pub episodes: usize,
}

#[derive(Debug, Serialize)]
pub struct ParsedMedia {
    pub parse_ok: bool,
    pub title: Option<String>,
    pub episode: Option<String>,
    pub episode_alt: Option<String>,
    pub episode_title: Option<String>,
    pub season: Option<String>,
    pub year: Option<String>,
    pub release_group: Option<String>,
    pub resolution: Option<String>,
    pub source: Option<String>,
    pub audio_term: Option<String>,
    pub video_term: Option<String>,
    pub subtitles: Option<String>,
    pub language: Option<String>,
    pub raw_elements: Vec<ParsedElement>,
}

#[derive(Debug, Serialize)]
pub struct ParsedElement {
    pub category: String,
    pub value: String,
}

pub async fn init_library(db: &PgPool) -> Result<(), LibraryError> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS media_files ( \
            id TEXT PRIMARY KEY, \
            path TEXT NOT NULL UNIQUE, \
            filename TEXT NOT NULL, \
            size BIGINT NOT NULL, \
            modified_at BIGINT NOT NULL, \
            last_seen_token TEXT, \
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), \
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW() \
        )",
    )
    .execute(db)
    .await?;

    sqlx::query("ALTER TABLE media_files ADD COLUMN IF NOT EXISTS last_seen_token TEXT")
        .execute(db)
        .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS media_parses ( \
            media_id TEXT PRIMARY KEY REFERENCES media_files(id) ON DELETE CASCADE, \
            parse_ok BOOLEAN NOT NULL, \
            title TEXT, \
            episode TEXT, \
            episode_alt TEXT, \
            episode_title TEXT, \
            season TEXT, \
            year TEXT, \
            release_group TEXT, \
            resolution TEXT, \
            source TEXT, \
            audio_term TEXT, \
            video_term TEXT, \
            subtitles TEXT, \
            language TEXT, \
            raw_elements JSONB NOT NULL, \
            parsed_at TIMESTAMPTZ NOT NULL DEFAULT NOW() \
        )",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS bangumi_subjects ( \
            id BIGINT PRIMARY KEY, \
            subject_type INTEGER NOT NULL, \
            name TEXT NOT NULL, \
            name_cn TEXT NOT NULL, \
            summary TEXT NOT NULL, \
            air_date TEXT, \
            total_episodes INTEGER, \
            images JSONB, \
            payload JSONB NOT NULL, \
            synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), \
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW() \
        )",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS bangumi_episodes ( \
            id BIGINT PRIMARY KEY, \
            subject_id BIGINT NOT NULL REFERENCES bangumi_subjects(id) ON DELETE CASCADE, \
            episode_type INTEGER NOT NULL, \
            sort DOUBLE PRECISION NOT NULL, \
            ep DOUBLE PRECISION, \
            name TEXT NOT NULL, \
            name_cn TEXT NOT NULL, \
            air_date TEXT, \
            payload JSONB NOT NULL, \
            synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), \
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW() \
        )",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS match_candidates ( \
            media_id TEXT NOT NULL REFERENCES media_files(id) ON DELETE CASCADE, \
            subject_id BIGINT NOT NULL REFERENCES bangumi_subjects(id) ON DELETE CASCADE, \
            confidence DOUBLE PRECISION NOT NULL, \
            reason TEXT NOT NULL, \
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), \
            PRIMARY KEY (media_id, subject_id) \
        )",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS media_matches ( \
            media_id TEXT PRIMARY KEY REFERENCES media_files(id) ON DELETE CASCADE, \
            subject_id BIGINT NOT NULL REFERENCES bangumi_subjects(id) ON DELETE CASCADE, \
            episode_id BIGINT REFERENCES bangumi_episodes(id) ON DELETE SET NULL, \
            method TEXT NOT NULL, \
            confidence DOUBLE PRECISION, \
            reason TEXT, \
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW() \
        )",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS job_queue ( \
            id BIGSERIAL PRIMARY KEY, \
            job_type TEXT NOT NULL, \
            status TEXT NOT NULL, \
            payload JSONB NOT NULL, \
            attempts INTEGER NOT NULL DEFAULT 0, \
            max_attempts INTEGER NOT NULL DEFAULT 3, \
            scheduled_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), \
            locked_at TIMESTAMPTZ, \
            locked_by TEXT, \
            dedup_key TEXT, \
            result JSONB, \
            last_error TEXT, \
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(), \
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW() \
        )",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS job_queue_status_idx ON job_queue (status, scheduled_at)",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS job_queue_dedup_active \
         ON job_queue (job_type, dedup_key) \
         WHERE status IN ('queued', 'running', 'retry')",
    )
    .execute(db)
    .await?;

    Ok(())
}

pub async fn scan_and_index(db: &PgPool, config: &MediaConfig) -> Result<IndexSummary, LibraryError> {
    let entries = scan_media(config)?;
    let scan_token = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
        .to_string();
    let mut summary = IndexSummary {
        scanned: entries.len(),
        upserted: 0,
        parsed: 0,
        skipped: 0,
        removed: 0,
    };

    let mut tx = db.begin().await?;
    for entry in entries {
        let modified_at = modified_epoch(&entry.path)?;
        let needs_parse = match fetch_media_meta(&mut tx, &entry.id).await? {
            Some(meta) => meta.size != entry.size as i64 || meta.modified_at != modified_at,
            None => true,
        };

        upsert_media_file(&mut tx, &entry, modified_at, &scan_token).await?;
        summary.upserted += 1;

        if needs_parse {
            let parsed = {
                let mut parser = Anitomy::new();
                parse_entry(&mut parser, &entry)
            };
            upsert_parse(&mut tx, &entry.id, &parsed).await?;
            summary.parsed += 1;
        } else {
            summary.skipped += 1;
        }
    }

    summary.removed = delete_stale_media(&mut tx, &scan_token).await?;
    tx.commit().await?;
    info!(
        scanned = summary.scanned,
        upserted = summary.upserted,
        parsed = summary.parsed,
        skipped = summary.skipped,
        removed = summary.removed,
        "library index complete"
    );

    Ok(summary)
}

pub async fn sync_bangumi_subject(
    db: &PgPool,
    client: &BangumiClient,
    subject_id: i64,
) -> Result<BangumiSyncSummary, LibraryError> {
    let subject = client.get_subject(subject_id).await?;
    let episodes = client.get_all_episodes(subject_id).await?;

    let mut tx = db.begin().await?;
    upsert_subject(&mut tx, &subject).await?;

    let mut synced = 0;
    for episode in &episodes {
        upsert_episode(&mut tx, &subject, episode).await?;
        synced += 1;
    }

    tx.commit().await?;
    info!(
        subject_id = subject.id,
        episodes = synced,
        "bangumi subject synced"
    );

    Ok(BangumiSyncSummary {
        subject_id: subject.id,
        episodes: synced,
    })
}

pub async fn auto_match_all(
    db: &PgPool,
    client: &BangumiClient,
    options: AutoMatchOptions,
) -> Result<AutoMatchSummary, LibraryError> {
    let rows = sqlx::query_as::<_, MediaParseRow>(
        "SELECT media_id, title, episode, year, parse_ok FROM media_parses",
    )
    .fetch_all(db)
    .await?;

    let mut summary = AutoMatchSummary::default();

    for row in rows {
        summary.scanned += 1;
        if !row.parse_ok {
            summary.skipped += 1;
            continue;
        }
        if has_manual_match(db, &row.media_id).await? {
            summary.skipped += 1;
            continue;
        }

        let title = match row.title {
            Some(value) if !value.trim().is_empty() => value,
            _ => {
                summary.skipped += 1;
                continue;
            }
        };

        let search = client.search_anime(&title, options.limit).await?;
        clear_candidates(db, &row.media_id).await?;
        clear_auto_match(db, &row.media_id).await?;

        if search.data.is_empty() {
            summary.skipped += 1;
            continue;
        }

        let mut best: Option<(Subject, f32, String)> = None;
        for subject in search.data {
            upsert_subject_cached(db, &subject).await?;
            let (score, reason) = score_subject(&title, row.year.as_deref(), &subject);
            if score < options.min_candidate_score {
                continue;
            }

            upsert_candidate(db, &row.media_id, subject.id, score, &reason).await?;
            summary.candidates += 1;

            if best.as_ref().map(|(_, s, _)| score > *s).unwrap_or(true) {
                best = Some((subject, score, reason));
            }
        }

        let Some((subject, score, reason)) = best else {
            summary.skipped += 1;
            continue;
        };

        if score < options.min_confidence {
            continue;
        }

        let episode_id = match row.episode.as_deref() {
            Some(ep_str) => {
                ensure_episode_cache(db, client, subject.id).await?;
                let episodes = load_cached_episodes(db, subject.id).await?;
                match_episode_id(ep_str, &episodes)
            }
            None => None,
        };

        upsert_media_match(
            db,
            &row.media_id,
            subject.id,
            episode_id,
            "auto",
            Some(score),
            Some(reason),
        )
        .await?;
        summary.matched += 1;
    }

    Ok(summary)
}

pub async fn set_manual_match(
    db: &PgPool,
    media_id: &str,
    subject_id: i64,
    episode_id: Option<i64>,
) -> Result<(), LibraryError> {
    ensure_subject_exists(db, subject_id).await?;
    if let Some(episode_id) = episode_id {
        ensure_episode_exists(db, subject_id, episode_id).await?;
    }
    upsert_media_match(
        db,
        media_id,
        subject_id,
        episode_id,
        "manual",
        None,
        Some("manual override".to_string()),
    )
    .await?;
    Ok(())
}

pub async fn clear_match(db: &PgPool, media_id: &str) -> Result<(), LibraryError> {
    sqlx::query("DELETE FROM media_matches WHERE media_id = $1")
        .bind(media_id)
        .execute(db)
        .await?;
    Ok(())
}

pub async fn get_match(db: &PgPool, media_id: &str) -> Result<Option<MediaMatch>, LibraryError> {
    let match_row = sqlx::query_as::<_, MediaMatchRow>(
        "SELECT media_id, subject_id, episode_id, method, confidence, reason \
         FROM media_matches WHERE media_id = $1",
    )
    .bind(media_id)
    .fetch_optional(db)
    .await?;

    Ok(match_row.map(|row| MediaMatch {
        media_id: row.media_id,
        subject_id: row.subject_id,
        episode_id: row.episode_id,
        method: row.method,
        confidence: row.confidence.map(|value| value as f32),
        reason: row.reason,
    }))
}

pub async fn get_candidates(
    db: &PgPool,
    media_id: &str,
) -> Result<Vec<MatchCandidate>, LibraryError> {
    let rows = sqlx::query_as::<_, MatchCandidateRow>(
        "SELECT c.subject_id, c.confidence, c.reason, s.name, s.name_cn \
         FROM match_candidates c \
         JOIN bangumi_subjects s ON s.id = c.subject_id \
         WHERE c.media_id = $1 \
         ORDER BY c.confidence DESC",
    )
    .bind(media_id)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| MatchCandidate {
            subject_id: row.subject_id,
            confidence: row.confidence as f32,
            reason: row.reason,
            name: row.name,
            name_cn: row.name_cn,
        })
        .collect())
}

pub async fn list_media_entries(db: &PgPool) -> Result<Vec<MediaEntry>, LibraryError> {
    let rows = sqlx::query_as::<_, (String, String, i64, String)>(
        "SELECT id, filename, size, path FROM media_files ORDER BY filename",
    )
    .fetch_all(db)
    .await?;

    rows.into_iter()
        .map(|row| {
            let path = Path::new(&row.3);
            Ok(MediaEntry {
                id: row.0,
                filename: row.1,
                size: row.2 as u64,
                path: path.to_path_buf(),
            })
        })
        .collect()
}

#[derive(Debug, Serialize)]
pub struct JobStatus {
    pub id: i64,
    pub job_type: String,
    pub status: String,
    pub attempts: i32,
    pub max_attempts: i32,
    pub result: Option<Value>,
    pub last_error: Option<String>,
}

#[derive(Debug)]
pub struct Job {
    pub id: i64,
    pub job_type: String,
    pub payload: Value,
    pub attempts: i32,
    pub max_attempts: i32,
}

pub async fn enqueue_job(
    db: &PgPool,
    job_type: &str,
    payload: Value,
    max_attempts: u32,
    dedup_key: Option<&str>,
) -> Result<i64, LibraryError> {
    let max_attempts = max_attempts as i32;
    let payload = sqlx::types::Json(payload);

    if let Some(key) = dedup_key {
        let row = sqlx::query_as::<_, (i64,)>(
            "INSERT INTO job_queue (job_type, status, payload, max_attempts, dedup_key) \
             VALUES ($1, 'queued', $2, $3, $4) \
             ON CONFLICT (job_type, dedup_key) WHERE status IN ('queued', 'running', 'retry') \
             DO NOTHING \
             RETURNING id",
        )
        .bind(job_type)
        .bind(&payload)
        .bind(max_attempts)
        .bind(key)
        .fetch_optional(db)
        .await?;

        if let Some(row) = row {
            return Ok(row.0);
        }

        if let Some(existing) = sqlx::query_as::<_, (i64,)>(
            "SELECT id FROM job_queue \
             WHERE job_type = $1 AND dedup_key = $2 AND status IN ('queued', 'running', 'retry') \
             ORDER BY id DESC LIMIT 1",
        )
        .bind(job_type)
        .bind(key)
        .fetch_optional(db)
        .await?
        {
            return Ok(existing.0);
        }
    }

    let row = sqlx::query_as::<_, (i64,)>(
        "INSERT INTO job_queue (job_type, status, payload, max_attempts) \
         VALUES ($1, 'queued', $2, $3) \
         RETURNING id",
    )
    .bind(job_type)
    .bind(&payload)
    .bind(max_attempts)
    .fetch_one(db)
    .await?;

    Ok(row.0)
}

pub async fn fetch_next_job(
    db: &PgPool,
    worker_id: &str,
) -> Result<Option<Job>, LibraryError> {
    let mut tx = db.begin().await?;

    let row = sqlx::query_as::<_, (i64, String, Value, i32, i32)>(
        "SELECT id, job_type, payload, attempts, max_attempts \
         FROM job_queue \
         WHERE status IN ('queued', 'retry') \
           AND scheduled_at <= NOW() \
           AND attempts < max_attempts \
         ORDER BY created_at \
         FOR UPDATE SKIP LOCKED \
         LIMIT 1",
    )
    .fetch_optional(&mut *tx)
    .await?;

    let Some(row) = row else {
        tx.commit().await?;
        return Ok(None);
    };

    let attempts = row.3 + 1;
    sqlx::query(
        "UPDATE job_queue \
         SET status = 'running', locked_at = NOW(), locked_by = $2, \
             attempts = $3, updated_at = NOW() \
         WHERE id = $1",
    )
    .bind(row.0)
    .bind(worker_id)
    .bind(attempts)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Some(Job {
        id: row.0,
        job_type: row.1,
        payload: row.2,
        attempts,
        max_attempts: row.4,
    }))
}

pub async fn complete_job(
    db: &PgPool,
    job_id: i64,
    result: Option<Value>,
) -> Result<(), LibraryError> {
    let result = result.map(sqlx::types::Json);
    sqlx::query(
        "UPDATE job_queue \
         SET status = 'done', result = $2, updated_at = NOW(), locked_at = NULL, locked_by = NULL \
         WHERE id = $1",
    )
    .bind(job_id)
    .bind(result)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn fail_job(
    db: &PgPool,
    job_id: i64,
    attempts: i32,
    max_attempts: i32,
    error: &str,
) -> Result<(), LibraryError> {
    if attempts >= max_attempts {
        sqlx::query(
            "UPDATE job_queue \
             SET status = 'failed', last_error = $2, updated_at = NOW(), locked_at = NULL, locked_by = NULL \
             WHERE id = $1",
        )
        .bind(job_id)
        .bind(error)
        .execute(db)
        .await?;
        return Ok(());
    }

    let delay_secs = 30 * (attempts as i64);
    sqlx::query(
        "UPDATE job_queue \
         SET status = 'retry', last_error = $2, \
             scheduled_at = NOW() + make_interval(secs => $3), \
             updated_at = NOW(), locked_at = NULL, locked_by = NULL \
         WHERE id = $1",
    )
    .bind(job_id)
    .bind(error)
    .bind(delay_secs)
    .execute(db)
    .await?;

    Ok(())
}

pub async fn get_job_status(db: &PgPool, job_id: i64) -> Result<Option<JobStatus>, LibraryError> {
    let row = sqlx::query_as::<_, (i64, String, String, i32, i32, Option<Value>, Option<String>)>(
        "SELECT id, job_type, status, attempts, max_attempts, result, last_error \
         FROM job_queue WHERE id = $1",
    )
    .bind(job_id)
    .fetch_optional(db)
    .await?;

    Ok(row.map(|row| JobStatus {
        id: row.0,
        job_type: row.1,
        status: row.2,
        attempts: row.3,
        max_attempts: row.4,
        result: row.5,
        last_error: row.6,
    }))
}

pub async fn cleanup_jobs(db: &PgPool, retention_hours: u64) -> Result<u64, LibraryError> {
    if retention_hours == 0 {
        return Ok(0);
    }
    let result = sqlx::query(
        "DELETE FROM job_queue \
         WHERE status IN ('done', 'failed') \
           AND updated_at < NOW() - make_interval(hours => $1)",
    )
    .bind(retention_hours as i64)
    .execute(db)
    .await?;
    Ok(result.rows_affected())
}

pub async fn requeue_stuck_jobs(
    db: &PgPool,
    timeout_secs: u64,
) -> Result<(u64, u64), LibraryError> {
    if timeout_secs == 0 {
        return Ok((0, 0));
    }
    let failed = sqlx::query(
        "UPDATE job_queue \
         SET status = 'failed', last_error = 'timeout', \
             updated_at = NOW(), locked_at = NULL, locked_by = NULL \
         WHERE status = 'running' \
           AND locked_at IS NOT NULL \
           AND locked_at < NOW() - make_interval(secs => $1) \
           AND attempts >= max_attempts",
    )
    .bind(timeout_secs as i64)
    .execute(db)
    .await?
    .rows_affected();

    let retried = sqlx::query(
        "UPDATE job_queue \
         SET status = 'retry', last_error = 'timeout', \
             scheduled_at = NOW(), updated_at = NOW(), locked_at = NULL, locked_by = NULL \
         WHERE status = 'running' \
           AND locked_at IS NOT NULL \
           AND locked_at < NOW() - make_interval(secs => $1) \
           AND attempts < max_attempts",
    )
    .bind(timeout_secs as i64)
    .execute(db)
    .await?
    .rows_affected();

    Ok((retried, failed))
}

#[derive(Debug, FromRow)]
struct MediaParseRow {
    media_id: String,
    title: Option<String>,
    episode: Option<String>,
    year: Option<String>,
    parse_ok: bool,
}

#[derive(Debug, FromRow)]
struct MediaFileMetaRow {
    size: i64,
    modified_at: i64,
}

#[derive(Debug, FromRow)]
struct MatchCandidateRow {
    subject_id: i64,
    confidence: f64,
    reason: String,
    name: String,
    name_cn: String,
}

#[derive(Debug, FromRow)]
struct MediaMatchRow {
    media_id: String,
    subject_id: i64,
    episode_id: Option<i64>,
    method: String,
    confidence: Option<f64>,
    reason: Option<String>,
}

pub fn parse_filename(filename: &str) -> ParsedMedia {
    let mut parser = Anitomy::new();
    let (parse_ok, elements) = match parser.parse(filename) {
        Ok(elements) => (true, elements),
        Err(elements) => (false, elements),
    };
    build_parsed_media(parse_ok, &elements)
}

fn parse_entry(parser: &mut Anitomy, entry: &MediaEntry) -> ParsedMedia {
    let filename = entry.filename.as_str();
    let (parse_ok, elements) = match parser.parse(filename) {
        Ok(elements) => (true, elements),
        Err(elements) => (false, elements),
    };
    build_parsed_media(parse_ok, &elements)
}

fn build_parsed_media(parse_ok: bool, elements: &Elements) -> ParsedMedia {
    let raw_elements = elements
        .iter()
        .map(|elem| ParsedElement {
            category: category_key(elem.category).to_string(),
            value: elem.value.clone(),
        })
        .collect::<Vec<_>>();

    ParsedMedia {
        parse_ok,
        title: get_element(elements, ElementCategory::AnimeTitle),
        episode: get_element(elements, ElementCategory::EpisodeNumber),
        episode_alt: get_element(elements, ElementCategory::EpisodeNumberAlt),
        episode_title: get_element(elements, ElementCategory::EpisodeTitle),
        season: get_element(elements, ElementCategory::AnimeSeason),
        year: get_element(elements, ElementCategory::AnimeYear),
        release_group: get_element(elements, ElementCategory::ReleaseGroup),
        resolution: get_element(elements, ElementCategory::VideoResolution),
        source: get_element(elements, ElementCategory::Source),
        audio_term: get_element(elements, ElementCategory::AudioTerm),
        video_term: get_element(elements, ElementCategory::VideoTerm),
        subtitles: join_elements(elements, ElementCategory::Subtitles),
        language: join_elements(elements, ElementCategory::Language),
        raw_elements,
    }
}

fn get_element(elements: &Elements, category: ElementCategory) -> Option<String> {
    elements.get(category).map(|value| value.to_string())
}

fn join_elements(elements: &Elements, category: ElementCategory) -> Option<String> {
    let values = elements.get_all(category);
    if values.is_empty() {
        None
    } else {
        Some(values.join(", "))
    }
}

async fn upsert_media_file(
    tx: &mut Transaction<'_, Postgres>,
    entry: &MediaEntry,
    modified_at: i64,
    scan_token: &str,
) -> Result<(), LibraryError> {
    let path = path_to_string(&entry.path)?;

    sqlx::query(
        "INSERT INTO media_files (id, path, filename, size, modified_at, last_seen_token) \
        VALUES ($1, $2, $3, $4, $5, $6) \
        ON CONFLICT (id) DO UPDATE SET \
            path = EXCLUDED.path, \
            filename = EXCLUDED.filename, \
            size = EXCLUDED.size, \
            modified_at = EXCLUDED.modified_at, \
            last_seen_token = EXCLUDED.last_seen_token, \
            updated_at = NOW()",
    )
    .bind(&entry.id)
    .bind(path)
    .bind(&entry.filename)
    .bind(entry.size as i64)
    .bind(modified_at)
    .bind(scan_token)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn upsert_parse(
    tx: &mut Transaction<'_, Postgres>,
    media_id: &str,
    parsed: &ParsedMedia,
) -> Result<(), LibraryError> {
    let raw_elements = sqlx::types::Json(&parsed.raw_elements);

    sqlx::query(
        "INSERT INTO media_parses ( \
            media_id, parse_ok, title, episode, episode_alt, episode_title, season, year, \
            release_group, resolution, source, audio_term, video_term, subtitles, language, raw_elements \
        ) VALUES ( \
            $1, $2, $3, $4, $5, $6, $7, $8, \
            $9, $10, $11, $12, $13, $14, $15, $16 \
        ) ON CONFLICT (media_id) DO UPDATE SET \
            parse_ok = EXCLUDED.parse_ok, \
            title = EXCLUDED.title, \
            episode = EXCLUDED.episode, \
            episode_alt = EXCLUDED.episode_alt, \
            episode_title = EXCLUDED.episode_title, \
            season = EXCLUDED.season, \
            year = EXCLUDED.year, \
            release_group = EXCLUDED.release_group, \
            resolution = EXCLUDED.resolution, \
            source = EXCLUDED.source, \
            audio_term = EXCLUDED.audio_term, \
            video_term = EXCLUDED.video_term, \
            subtitles = EXCLUDED.subtitles, \
            language = EXCLUDED.language, \
            raw_elements = EXCLUDED.raw_elements, \
            parsed_at = NOW()",
    )
    .bind(media_id)
    .bind(parsed.parse_ok)
    .bind(parsed.title.as_deref())
    .bind(parsed.episode.as_deref())
    .bind(parsed.episode_alt.as_deref())
    .bind(parsed.episode_title.as_deref())
    .bind(parsed.season.as_deref())
    .bind(parsed.year.as_deref())
    .bind(parsed.release_group.as_deref())
    .bind(parsed.resolution.as_deref())
    .bind(parsed.source.as_deref())
    .bind(parsed.audio_term.as_deref())
    .bind(parsed.video_term.as_deref())
    .bind(parsed.subtitles.as_deref())
    .bind(parsed.language.as_deref())
    .bind(raw_elements)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

fn path_to_string(path: &Path) -> Result<String, LibraryError> {
    path.to_str()
        .map(|value| value.to_string())
        .ok_or_else(|| LibraryError::InvalidPath(format!("non-utf8 path: {}", path.display())))
}

fn modified_epoch(path: &Path) -> Result<i64, LibraryError> {
    let metadata = fs::metadata(path)?;
    let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
    let duration = modified.duration_since(UNIX_EPOCH).unwrap_or_default();
    Ok(duration.as_secs() as i64)
}

async fn fetch_media_meta(
    tx: &mut Transaction<'_, Postgres>,
    media_id: &str,
) -> Result<Option<MediaFileMetaRow>, LibraryError> {
    let row = sqlx::query_as::<_, MediaFileMetaRow>(
        "SELECT size, modified_at FROM media_files WHERE id = $1",
    )
    .bind(media_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row)
}

async fn delete_stale_media(
    tx: &mut Transaction<'_, Postgres>,
    scan_token: &str,
) -> Result<usize, LibraryError> {
    let result = sqlx::query("DELETE FROM media_files WHERE last_seen_token IS DISTINCT FROM $1")
        .bind(scan_token)
        .execute(&mut **tx)
        .await?;
    Ok(result.rows_affected() as usize)
}

async fn upsert_subject(
    tx: &mut Transaction<'_, Postgres>,
    subject: &Subject,
) -> Result<(), LibraryError> {
    let payload = sqlx::types::Json(subject);

    sqlx::query(
        "INSERT INTO bangumi_subjects ( \
            id, subject_type, name, name_cn, summary, air_date, total_episodes, images, payload \
        ) VALUES ( \
            $1, $2, $3, $4, $5, $6, $7, $8, $9 \
        ) ON CONFLICT (id) DO UPDATE SET \
            subject_type = EXCLUDED.subject_type, \
            name = EXCLUDED.name, \
            name_cn = EXCLUDED.name_cn, \
            summary = EXCLUDED.summary, \
            air_date = EXCLUDED.air_date, \
            total_episodes = EXCLUDED.total_episodes, \
            images = EXCLUDED.images, \
            payload = EXCLUDED.payload, \
            synced_at = NOW(), \
            updated_at = NOW()",
    )
    .bind(subject.id)
    .bind(subject.subject_type)
    .bind(&subject.name)
    .bind(&subject.name_cn)
    .bind(&subject.summary)
    .bind(normalize_optional(&subject.date))
    .bind(subject.total_episodes.map(|value| value as i32))
    .bind(sqlx::types::Json(&subject.images))
    .bind(payload)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

async fn upsert_episode(
    tx: &mut Transaction<'_, Postgres>,
    subject: &Subject,
    episode: &Episode,
) -> Result<(), LibraryError> {
    let payload = sqlx::types::Json(episode);

    sqlx::query(
        "INSERT INTO bangumi_episodes ( \
            id, subject_id, episode_type, sort, ep, name, name_cn, air_date, payload \
        ) VALUES ( \
            $1, $2, $3, $4, $5, $6, $7, $8, $9 \
        ) ON CONFLICT (id) DO UPDATE SET \
            subject_id = EXCLUDED.subject_id, \
            episode_type = EXCLUDED.episode_type, \
            sort = EXCLUDED.sort, \
            ep = EXCLUDED.ep, \
            name = EXCLUDED.name, \
            name_cn = EXCLUDED.name_cn, \
            air_date = EXCLUDED.air_date, \
            payload = EXCLUDED.payload, \
            synced_at = NOW(), \
            updated_at = NOW()",
    )
    .bind(episode.id)
    .bind(subject.id)
    .bind(episode.episode_type)
    .bind(episode.sort)
    .bind(episode.ep)
    .bind(&episode.name)
    .bind(&episode.name_cn)
    .bind(normalize_optional(&episode.airdate))
    .bind(payload)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

fn normalize_optional(value: &Option<String>) -> Option<&str> {
    match value {
        Some(text) if text.trim().is_empty() => None,
        Some(text) => Some(text.as_str()),
        None => None,
    }
}

async fn upsert_subject_cached(db: &PgPool, subject: &Subject) -> Result<(), LibraryError> {
    let payload = sqlx::types::Json(subject);
    sqlx::query(
        "INSERT INTO bangumi_subjects ( \
            id, subject_type, name, name_cn, summary, air_date, total_episodes, images, payload \
        ) VALUES ( \
            $1, $2, $3, $4, $5, $6, $7, $8, $9 \
        ) ON CONFLICT (id) DO UPDATE SET \
            subject_type = EXCLUDED.subject_type, \
            name = EXCLUDED.name, \
            name_cn = EXCLUDED.name_cn, \
            summary = EXCLUDED.summary, \
            air_date = EXCLUDED.air_date, \
            total_episodes = EXCLUDED.total_episodes, \
            images = EXCLUDED.images, \
            payload = EXCLUDED.payload, \
            synced_at = NOW(), \
            updated_at = NOW()",
    )
    .bind(subject.id)
    .bind(subject.subject_type)
    .bind(&subject.name)
    .bind(&subject.name_cn)
    .bind(&subject.summary)
    .bind(normalize_optional(&subject.date))
    .bind(subject.total_episodes.map(|value| value as i32))
    .bind(sqlx::types::Json(&subject.images))
    .bind(payload)
    .execute(db)
    .await?;

    Ok(())
}

async fn upsert_candidate(
    db: &PgPool,
    media_id: &str,
    subject_id: i64,
    confidence: f32,
    reason: &str,
) -> Result<(), LibraryError> {
    sqlx::query(
        "INSERT INTO match_candidates (media_id, subject_id, confidence, reason) \
        VALUES ($1, $2, $3, $4) \
        ON CONFLICT (media_id, subject_id) DO UPDATE SET \
            confidence = EXCLUDED.confidence, \
            reason = EXCLUDED.reason, \
            created_at = NOW()",
    )
    .bind(media_id)
    .bind(subject_id)
    .bind(confidence as f64)
    .bind(reason)
    .execute(db)
    .await?;

    Ok(())
}

async fn clear_candidates(db: &PgPool, media_id: &str) -> Result<(), LibraryError> {
    sqlx::query("DELETE FROM match_candidates WHERE media_id = $1")
        .bind(media_id)
        .execute(db)
        .await?;
    Ok(())
}

async fn clear_auto_match(db: &PgPool, media_id: &str) -> Result<(), LibraryError> {
    sqlx::query("DELETE FROM media_matches WHERE media_id = $1 AND method = 'auto'")
        .bind(media_id)
        .execute(db)
        .await?;
    Ok(())
}

async fn has_manual_match(db: &PgPool, media_id: &str) -> Result<bool, LibraryError> {
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT 1 FROM media_matches WHERE media_id = $1 AND method = 'manual' LIMIT 1",
    )
    .bind(media_id)
    .fetch_optional(db)
    .await?
    .is_some();
    Ok(exists)
}

async fn upsert_media_match(
    db: &PgPool,
    media_id: &str,
    subject_id: i64,
    episode_id: Option<i64>,
    method: &str,
    confidence: Option<f32>,
    reason: Option<String>,
) -> Result<(), LibraryError> {
    sqlx::query(
        "INSERT INTO media_matches (media_id, subject_id, episode_id, method, confidence, reason) \
        VALUES ($1, $2, $3, $4, $5, $6) \
        ON CONFLICT (media_id) DO UPDATE SET \
            subject_id = EXCLUDED.subject_id, \
            episode_id = EXCLUDED.episode_id, \
            method = EXCLUDED.method, \
            confidence = EXCLUDED.confidence, \
            reason = EXCLUDED.reason, \
            updated_at = NOW()",
    )
    .bind(media_id)
    .bind(subject_id)
    .bind(episode_id)
    .bind(method)
    .bind(confidence.map(|value| value as f64))
    .bind(reason)
    .execute(db)
    .await?;

    Ok(())
}

async fn ensure_subject_exists(db: &PgPool, subject_id: i64) -> Result<(), LibraryError> {
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT 1 FROM bangumi_subjects WHERE id = $1 LIMIT 1",
    )
    .bind(subject_id)
    .fetch_optional(db)
    .await?
    .is_some();

    if !exists {
        return Err(LibraryError::InvalidInput(format!(
            "subject {} not cached",
            subject_id
        )));
    }

    Ok(())
}

async fn ensure_episode_exists(
    db: &PgPool,
    subject_id: i64,
    episode_id: i64,
) -> Result<(), LibraryError> {
    let exists = sqlx::query_scalar::<_, i64>(
        "SELECT 1 FROM bangumi_episodes WHERE id = $1 AND subject_id = $2 LIMIT 1",
    )
    .bind(episode_id)
    .bind(subject_id)
    .fetch_optional(db)
    .await?
    .is_some();

    if !exists {
        return Err(LibraryError::InvalidInput(format!(
            "episode {} not cached",
            episode_id
        )));
    }

    Ok(())
}

async fn ensure_episode_cache(
    db: &PgPool,
    client: &BangumiClient,
    subject_id: i64,
) -> Result<(), LibraryError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(1) FROM bangumi_episodes WHERE subject_id = $1",
    )
    .bind(subject_id)
    .fetch_one(db)
    .await?;

    if count == 0 {
        let _ = sync_bangumi_subject(db, client, subject_id).await?;
    }

    Ok(())
}

async fn load_cached_episodes(
    db: &PgPool,
    subject_id: i64,
) -> Result<Vec<Episode>, LibraryError> {
    let rows = sqlx::query_as::<_, EpisodeRow>(
        "SELECT id, episode_type, sort, ep, name, name_cn, air_date \
         FROM bangumi_episodes WHERE subject_id = $1",
    )
    .bind(subject_id)
    .fetch_all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| Episode {
            id: row.id,
            episode_type: row.episode_type,
            sort: row.sort,
            ep: row.ep,
            name: row.name,
            name_cn: row.name_cn,
            airdate: row.air_date,
        })
        .collect())
}

#[derive(Debug, FromRow)]
struct EpisodeRow {
    id: i64,
    episode_type: i32,
    sort: f64,
    ep: Option<f64>,
    name: String,
    name_cn: String,
    air_date: Option<String>,
}

fn match_episode_id(episode_str: &str, episodes: &[Episode]) -> Option<i64> {
    let target = parse_episode_number(episode_str)?;
    let mut best: Option<(i64, f64)> = None;
    for episode in episodes {
        let value = episode.ep.unwrap_or(episode.sort);
        let diff = (value - target).abs();
        if diff <= 0.01 {
            return Some(episode.id);
        }
        if best
            .as_ref()
            .map(|(_, best_diff)| diff < *best_diff)
            .unwrap_or(true)
        {
            best = Some((episode.id, diff));
        }
    }

    best.and_then(|(id, diff)| if diff <= 1.0 { Some(id) } else { None })
}

fn parse_episode_number(raw: &str) -> Option<f64> {
    let mut buf = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            buf.push(ch);
        } else if !buf.is_empty() {
            break;
        }
    }
    if buf.is_empty() {
        None
    } else {
        buf.parse::<f64>().ok()
    }
}

fn score_subject(title: &str, year: Option<&str>, subject: &Subject) -> (f32, String) {
    let mut reason = String::new();
    let base = title_similarity(title, &subject.name, &subject.name_cn);
    reason.push_str(&format!("title={:.2}", base));
    let mut score = base;

    if let (Some(year), Some(date)) = (year, subject.date.as_deref()) {
        if date.starts_with(year) {
            score = (score + 0.05).min(1.0);
            reason.push_str(";year=+0.05");
        }
    }

    (score, reason)
}

fn title_similarity(title: &str, name: &str, name_cn: &str) -> f32 {
    let score_name = similarity(normalize_title(title).as_str(), normalize_title(name).as_str());
    let score_cn = if name_cn.trim().is_empty() {
        0.0
    } else {
        similarity(
            normalize_title(title).as_str(),
            normalize_title(name_cn).as_str(),
        )
    };
    score_name.max(score_cn)
}

fn normalize_title(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn similarity(a: &str, b: &str) -> f32 {
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    if a == b {
        return 1.0;
    }
    if a.contains(b) || b.contains(a) {
        return 0.85;
    }
    let a_bigrams = bigrams(a);
    let b_bigrams = bigrams(b);
    if a_bigrams.is_empty() || b_bigrams.is_empty() {
        return 0.0;
    }
    let mut matches = 0;
    let mut b_used = vec![false; b_bigrams.len()];
    for a_bg in &a_bigrams {
        for (idx, b_bg) in b_bigrams.iter().enumerate() {
            if !b_used[idx] && a_bg == b_bg {
                matches += 1;
                b_used[idx] = true;
                break;
            }
        }
    }
    (2.0 * matches as f32) / (a_bigrams.len() as f32 + b_bigrams.len() as f32)
}

fn bigrams(value: &str) -> Vec<String> {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() < 2 {
        return Vec::new();
    }
    chars
        .windows(2)
        .map(|pair| pair.iter().collect())
        .collect()
}

fn category_key(category: ElementCategory) -> &'static str {
    match category {
        ElementCategory::AnimeSeason => "anime_season",
        ElementCategory::AnimeSeasonPrefix => "anime_season_prefix",
        ElementCategory::AnimeTitle => "anime_title",
        ElementCategory::AnimeType => "anime_type",
        ElementCategory::AnimeYear => "anime_year",
        ElementCategory::AudioTerm => "audio_term",
        ElementCategory::DeviceCompatibility => "device_compatibility",
        ElementCategory::EpisodeNumber => "episode_number",
        ElementCategory::EpisodeNumberAlt => "episode_number_alt",
        ElementCategory::EpisodePrefix => "episode_prefix",
        ElementCategory::EpisodeTitle => "episode_title",
        ElementCategory::FileChecksum => "file_checksum",
        ElementCategory::FileExtension => "file_extension",
        ElementCategory::FileName => "file_name",
        ElementCategory::Language => "language",
        ElementCategory::Other => "other",
        ElementCategory::ReleaseGroup => "release_group",
        ElementCategory::ReleaseInformation => "release_information",
        ElementCategory::ReleaseVersion => "release_version",
        ElementCategory::Source => "source",
        ElementCategory::Subtitles => "subtitles",
        ElementCategory::VideoResolution => "video_resolution",
        ElementCategory::VideoTerm => "video_term",
        ElementCategory::VolumeNumber => "volume_number",
        ElementCategory::VolumePrefix => "volume_prefix",
        ElementCategory::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_episode_number_handles_suffix() {
        assert_eq!(parse_episode_number("12v2"), Some(12.0));
        assert_eq!(parse_episode_number("03"), Some(3.0));
        assert_eq!(parse_episode_number("SP"), None);
    }

    #[test]
    fn match_episode_id_prefers_exact_match() {
        let episodes = vec![
            Episode {
                id: 100,
                episode_type: 0,
                name: "Ep 11".to_string(),
                name_cn: "".to_string(),
                sort: 11.0,
                ep: Some(11.0),
                airdate: None,
            },
            Episode {
                id: 101,
                episode_type: 0,
                name: "Ep 12".to_string(),
                name_cn: "".to_string(),
                sort: 12.0,
                ep: Some(12.0),
                airdate: None,
            },
        ];
        assert_eq!(match_episode_id("12", &episodes), Some(101));
    }

    #[test]
    fn similarity_handles_basic_cases() {
        assert_eq!(similarity("abc", "abc"), 1.0);
        assert_eq!(similarity("", "abc"), 0.0);
        assert!(similarity("spyxfamily", "spyfamily") > 0.5);
    }

    #[test]
    fn normalize_title_removes_separators() {
        let normalized = normalize_title("Spy x Family!!");
        assert_eq!(normalized, "spyxfamily");
    }
}
