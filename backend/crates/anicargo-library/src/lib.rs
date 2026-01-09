use anicargo_bangumi::{BangumiClient, BangumiError, Episode, Subject};
use anicargo_media::{scan_media, MediaConfig, MediaEntry, MediaError};
use anitomy::{Anitomy, ElementCategory, Elements};
use serde::Serialize;
use sqlx::{PgPool, Postgres, Transaction};
use std::fmt;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;
use tracing::info;

#[derive(Debug)]
pub enum LibraryError {
    Media(MediaError),
    Sql(sqlx::Error),
    Io(std::io::Error),
    InvalidPath(String),
    Bangumi(BangumiError),
}

impl fmt::Display for LibraryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LibraryError::Media(err) => write!(f, "media error: {}", err),
            LibraryError::Sql(err) => write!(f, "database error: {}", err),
            LibraryError::Io(err) => write!(f, "io error: {}", err),
            LibraryError::InvalidPath(message) => write!(f, "invalid path: {}", message),
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
        "CREATE TABLE IF NOT EXISTS media_files (\
            id TEXT PRIMARY KEY,\
            path TEXT NOT NULL UNIQUE,\
            filename TEXT NOT NULL,\
            size BIGINT NOT NULL,\
            modified_at BIGINT NOT NULL,\
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),\
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()\
        )",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS media_parses (\
            media_id TEXT PRIMARY KEY REFERENCES media_files(id) ON DELETE CASCADE,\
            parse_ok BOOLEAN NOT NULL,\
            title TEXT,\
            episode TEXT,\
            episode_alt TEXT,\
            episode_title TEXT,\
            season TEXT,\
            year TEXT,\
            release_group TEXT,\
            resolution TEXT,\
            source TEXT,\
            audio_term TEXT,\
            video_term TEXT,\
            subtitles TEXT,\
            language TEXT,\
            raw_elements JSONB NOT NULL,\
            parsed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()\
        )",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS bangumi_subjects (\
            id BIGINT PRIMARY KEY,\
            subject_type INTEGER NOT NULL,\
            name TEXT NOT NULL,\
            name_cn TEXT NOT NULL,\
            summary TEXT NOT NULL,\
            air_date TEXT,\
            total_episodes INTEGER,\
            images JSONB,\
            payload JSONB NOT NULL,\
            synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),\
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()\
        )",
    )
    .execute(db)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS bangumi_episodes (\
            id BIGINT PRIMARY KEY,\
            subject_id BIGINT NOT NULL REFERENCES bangumi_subjects(id) ON DELETE CASCADE,\
            episode_type INTEGER NOT NULL,\
            sort DOUBLE PRECISION NOT NULL,\
            ep DOUBLE PRECISION,\
            name TEXT NOT NULL,\
            name_cn TEXT NOT NULL,\
            air_date TEXT,\
            payload JSONB NOT NULL,\
            synced_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),\
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()\
        )",
    )
    .execute(db)
    .await?;

    Ok(())
}

pub async fn scan_and_index(db: &PgPool, config: &MediaConfig) -> Result<IndexSummary, LibraryError> {
    let entries = scan_media(config)?;
    let mut summary = IndexSummary {
        scanned: entries.len(),
        upserted: 0,
        parsed: 0,
    };

    let mut tx = db.begin().await?;
    let mut parser = Anitomy::new();

    for entry in entries {
        upsert_media_file(&mut tx, &entry).await?;
        summary.upserted += 1;

        let parsed = parse_entry(&mut parser, &entry);
        upsert_parse(&mut tx, &entry.id, &parsed).await?;
        summary.parsed += 1;
    }

    tx.commit().await?;
    info!(
        scanned = summary.scanned,
        upserted = summary.upserted,
        parsed = summary.parsed,
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

fn parse_entry(parser: &mut Anitomy, entry: &MediaEntry) -> ParsedMedia {
    let filename = entry.filename.as_str();
    let (parse_ok, elements) = match parser.parse(filename) {
        Ok(elements) => (true, elements),
        Err(elements) => (false, elements),
    };

    let raw_elements = elements
        .iter()
        .map(|elem| ParsedElement {
            category: category_key(elem.category).to_string(),
            value: elem.value.clone(),
        })
        .collect::<Vec<_>>();

    ParsedMedia {
        parse_ok,
        title: get_element(&elements, ElementCategory::AnimeTitle),
        episode: get_element(&elements, ElementCategory::EpisodeNumber),
        episode_alt: get_element(&elements, ElementCategory::EpisodeNumberAlt),
        episode_title: get_element(&elements, ElementCategory::EpisodeTitle),
        season: get_element(&elements, ElementCategory::AnimeSeason),
        year: get_element(&elements, ElementCategory::AnimeYear),
        release_group: get_element(&elements, ElementCategory::ReleaseGroup),
        resolution: get_element(&elements, ElementCategory::VideoResolution),
        source: get_element(&elements, ElementCategory::Source),
        audio_term: get_element(&elements, ElementCategory::AudioTerm),
        video_term: get_element(&elements, ElementCategory::VideoTerm),
        subtitles: join_elements(&elements, ElementCategory::Subtitles),
        language: join_elements(&elements, ElementCategory::Language),
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
) -> Result<(), LibraryError> {
    let path = path_to_string(&entry.path)?;
    let modified_at = modified_epoch(&entry.path)?;

    sqlx::query(
        "INSERT INTO media_files (id, path, filename, size, modified_at)\
        VALUES ($1, $2, $3, $4, $5)\
        ON CONFLICT (id) DO UPDATE SET\
            path = EXCLUDED.path,\
            filename = EXCLUDED.filename,\
            size = EXCLUDED.size,\
            modified_at = EXCLUDED.modified_at,\
            updated_at = NOW()",
    )
    .bind(&entry.id)
    .bind(path)
    .bind(&entry.filename)
    .bind(entry.size as i64)
    .bind(modified_at)
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
        "INSERT INTO media_parses (\
            media_id, parse_ok, title, episode, episode_alt, episode_title, season, year,\
            release_group, resolution, source, audio_term, video_term, subtitles, language, raw_elements\
        ) VALUES (\
            $1, $2, $3, $4, $5, $6, $7, $8,\
            $9, $10, $11, $12, $13, $14, $15, $16\
        ) ON CONFLICT (media_id) DO UPDATE SET\
            parse_ok = EXCLUDED.parse_ok,\
            title = EXCLUDED.title,\
            episode = EXCLUDED.episode,\
            episode_alt = EXCLUDED.episode_alt,\
            episode_title = EXCLUDED.episode_title,\
            season = EXCLUDED.season,\
            year = EXCLUDED.year,\
            release_group = EXCLUDED.release_group,\
            resolution = EXCLUDED.resolution,\
            source = EXCLUDED.source,\
            audio_term = EXCLUDED.audio_term,\
            video_term = EXCLUDED.video_term,\
            subtitles = EXCLUDED.subtitles,\
            language = EXCLUDED.language,\
            raw_elements = EXCLUDED.raw_elements,\
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

async fn upsert_subject(
    tx: &mut Transaction<'_, Postgres>,
    subject: &Subject,
) -> Result<(), LibraryError> {
    let payload = sqlx::types::Json(subject);

    sqlx::query(
        "INSERT INTO bangumi_subjects (\
            id, subject_type, name, name_cn, summary, air_date, total_episodes, images, payload\
        ) VALUES (\
            $1, $2, $3, $4, $5, $6, $7, $8, $9\
        ) ON CONFLICT (id) DO UPDATE SET\
            subject_type = EXCLUDED.subject_type,\
            name = EXCLUDED.name,\
            name_cn = EXCLUDED.name_cn,\
            summary = EXCLUDED.summary,\
            air_date = EXCLUDED.air_date,\
            total_episodes = EXCLUDED.total_episodes,\
            images = EXCLUDED.images,\
            payload = EXCLUDED.payload,\
            synced_at = NOW(),\
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
        "INSERT INTO bangumi_episodes (\
            id, subject_id, episode_type, sort, ep, name, name_cn, air_date, payload\
        ) VALUES (\
            $1, $2, $3, $4, $5, $6, $7, $8, $9\
        ) ON CONFLICT (id) DO UPDATE SET\
            subject_id = EXCLUDED.subject_id,\
            episode_type = EXCLUDED.episode_type,\
            sort = EXCLUDED.sort,\
            ep = EXCLUDED.ep,\
            name = EXCLUDED.name,\
            name_cn = EXCLUDED.name_cn,\
            air_date = EXCLUDED.air_date,\
            payload = EXCLUDED.payload,\
            synced_at = NOW(),\
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
