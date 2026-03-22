use std::{fs, path::Path};

use anyhow::Context;
use chrono::{Duration, Utc};
use sqlx::{
    FromRow, SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};

use crate::{
    auth::{AdminIdentity, ViewerIdentity, generate_token, hash_password, verify_password},
    config::{AppConfig, AuthConfig},
    types::{
        AdminCountsDto, AppError, DownloadExecutionDto, DownloadExecutionEventDto, DownloadJobDto,
        FansubRuleDto, PolicyDto, ResourceCandidateDto, ResourceLibraryItemDto,
        SubjectDownloadStatusDto,
    },
};

#[derive(Debug, FromRow)]
struct UserRow {
    id: i64,
    username: String,
    password_hash: String,
}

#[derive(Debug, FromRow)]
struct AdminRow {
    id: i64,
    username: String,
    password_hash: String,
}

#[derive(Debug, FromRow)]
struct PolicyRow {
    subscription_threshold: i64,
    replacement_window_hours: i64,
    prefer_same_fansub: i64,
    max_concurrent_downloads: i64,
    upload_limit_mb: i64,
    download_limit_mb: i64,
}

#[derive(Debug, FromRow)]
struct DownloadSubjectRow {
    bangumi_subject_id: i64,
    release_status: String,
    demand_state: String,
    subscription_count: i64,
    threshold_snapshot: i64,
    last_queued_job_id: Option<i64>,
    last_evaluated_at: String,
}

#[derive(Debug, FromRow)]
struct FansubRuleRow {
    id: i64,
    fansub_name: String,
    locale_preference: String,
    priority: i64,
    is_blacklist: i64,
}

#[derive(Debug, FromRow)]
struct DownloadJobRow {
    id: i64,
    bangumi_subject_id: i64,
    trigger_kind: String,
    requested_by: String,
    release_status: String,
    season_mode: String,
    lifecycle: String,
    subscription_count: i64,
    threshold_snapshot: i64,
    engine_name: String,
    engine_job_ref: Option<String>,
    notes: Option<String>,
    selected_candidate_id: Option<i64>,
    selection_updated_at: Option<String>,
    last_search_run_id: Option<i64>,
    search_status: String,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, FromRow)]
struct ResourceCandidateRow {
    id: i64,
    download_job_id: i64,
    search_run_id: i64,
    bangumi_subject_id: i64,
    slot_key: String,
    episode_index: Option<f64>,
    episode_end_index: Option<f64>,
    is_collection: i64,
    provider: String,
    provider_resource_id: String,
    title: String,
    href: String,
    magnet: String,
    release_type: String,
    size_bytes: i64,
    fansub_name: Option<String>,
    publisher_name: String,
    source_created_at: String,
    source_fetched_at: String,
    resolution: Option<String>,
    locale_hint: Option<String>,
    is_raw: i64,
    score: f64,
    rejected_reason: Option<String>,
    discovered_at: String,
}

#[derive(Debug, FromRow)]
struct DownloadExecutionRow {
    id: i64,
    download_job_id: i64,
    resource_candidate_id: i64,
    bangumi_subject_id: i64,
    slot_key: String,
    episode_index: Option<f64>,
    episode_end_index: Option<f64>,
    is_collection: i64,
    engine_name: String,
    engine_execution_ref: Option<String>,
    execution_role: String,
    state: String,
    target_path: String,
    source_title: String,
    source_magnet: String,
    source_size_bytes: i64,
    source_fansub_name: Option<String>,
    downloaded_bytes: i64,
    uploaded_bytes: i64,
    download_rate_bytes: i64,
    upload_rate_bytes: i64,
    peer_count: i64,
    notes: Option<String>,
    created_at: String,
    updated_at: String,
    started_at: Option<String>,
    completed_at: Option<String>,
    replaced_at: Option<String>,
    failed_at: Option<String>,
    last_indexed_at: Option<String>,
}

#[derive(Debug, FromRow)]
struct DownloadExecutionEventRow {
    id: i64,
    download_execution_id: i64,
    level: String,
    event_kind: String,
    message: String,
    downloaded_bytes: Option<i64>,
    uploaded_bytes: Option<i64>,
    download_rate_bytes: Option<i64>,
    upload_rate_bytes: Option<i64>,
    peer_count: Option<i64>,
    created_at: String,
}

#[derive(Debug, FromRow)]
struct ResourceLibraryRow {
    id: i64,
    bangumi_subject_id: i64,
    download_job_id: i64,
    download_execution_id: i64,
    resource_candidate_id: i64,
    slot_key: String,
    source_title: String,
    source_fansub_name: Option<String>,
    execution_state: String,
    relative_path: String,
    absolute_path: String,
    file_name: String,
    file_ext: String,
    size_bytes: i64,
    episode_index: Option<f64>,
    episode_end_index: Option<f64>,
    is_collection: i64,
    status: String,
    updated_at: String,
}

#[derive(Debug, FromRow)]
struct SubjectEpisodeAvailabilityRow {
    episode_index: Option<f64>,
    episode_end_index: Option<f64>,
    is_collection: i64,
    status: String,
}

#[derive(Debug, FromRow)]
struct ViewerSubscriptionRow {
    bangumi_subject_id: i64,
    subscribed_at: String,
    latest_ready_at: Option<String>,
}

#[derive(Debug, FromRow)]
struct PlaybackHistoryRow {
    bangumi_subject_id: i64,
    bangumi_episode_id: i64,
    file_name: Option<String>,
    source_fansub_name: Option<String>,
    last_played_at: String,
    play_count: i64,
}

#[derive(Debug, FromRow)]
struct CachedBangumiSubjectSummaryRow {
    title: String,
    title_cn: String,
    release_status: String,
}

pub struct NewDownloadJob {
    pub bangumi_subject_id: i64,
    pub trigger_kind: String,
    pub requested_by: String,
    pub release_status: String,
    pub season_mode: String,
    pub lifecycle: String,
    pub subscription_count: i64,
    pub threshold_snapshot: i64,
    pub engine_name: String,
    pub engine_job_ref: Option<String>,
    pub notes: Option<String>,
}

pub struct NewResourceCandidate {
    pub download_job_id: i64,
    pub search_run_id: i64,
    pub bangumi_subject_id: i64,
    pub slot_key: String,
    pub episode_index: Option<f64>,
    pub episode_end_index: Option<f64>,
    pub is_collection: bool,
    pub provider: String,
    pub provider_resource_id: String,
    pub title: String,
    pub href: String,
    pub magnet: String,
    pub release_type: String,
    pub size_bytes: i64,
    pub fansub_name: Option<String>,
    pub publisher_name: String,
    pub source_created_at: String,
    pub source_fetched_at: String,
    pub resolution: Option<String>,
    pub locale_hint: Option<String>,
    pub is_raw: bool,
    pub score: f64,
    pub rejected_reason: Option<String>,
}

pub struct NewDownloadExecution {
    pub download_job_id: i64,
    pub resource_candidate_id: i64,
    pub bangumi_subject_id: i64,
    pub slot_key: String,
    pub episode_index: Option<f64>,
    pub episode_end_index: Option<f64>,
    pub is_collection: bool,
    pub engine_name: String,
    pub engine_execution_ref: Option<String>,
    pub execution_role: String,
    pub state: String,
    pub target_path: String,
    pub source_title: String,
    pub source_magnet: String,
    pub source_size_bytes: i64,
    pub source_fansub_name: Option<String>,
    pub downloaded_bytes: i64,
    pub uploaded_bytes: i64,
    pub download_rate_bytes: i64,
    pub upload_rate_bytes: i64,
    pub peer_count: i64,
    pub notes: Option<String>,
}

pub struct NewDownloadExecutionEvent {
    pub download_execution_id: i64,
    pub level: String,
    pub event_kind: String,
    pub message: String,
    pub downloaded_bytes: Option<i64>,
    pub uploaded_bytes: Option<i64>,
    pub download_rate_bytes: Option<i64>,
    pub upload_rate_bytes: Option<i64>,
    pub peer_count: Option<i64>,
}

pub struct NewMediaInventoryItem {
    pub bangumi_subject_id: i64,
    pub download_job_id: i64,
    pub download_execution_id: i64,
    pub resource_candidate_id: i64,
    pub slot_key: String,
    pub relative_path: String,
    pub absolute_path: String,
    pub file_name: String,
    pub file_ext: String,
    pub size_bytes: i64,
    pub episode_index: Option<f64>,
    pub episode_end_index: Option<f64>,
    pub is_collection: bool,
    pub status: String,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeOverview {
    pub devices: i64,
    pub users: i64,
    pub active_sessions: i64,
    pub subscriptions: i64,
    pub open_download_jobs: i64,
    pub jobs_with_selection: i64,
    pub running_searches: i64,
    pub resource_candidates: i64,
    pub active_executions: i64,
    pub downloaded_bytes: i64,
    pub uploaded_bytes: i64,
    pub download_rate_bytes: i64,
    pub upload_rate_bytes: i64,
    pub peer_count: i64,
}

#[derive(Debug, Clone)]
pub struct SubjectEpisodeAvailability {
    pub episode_index: Option<f64>,
    pub episode_end_index: Option<f64>,
    pub is_collection: bool,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct ViewerSubscriptionEntry {
    pub bangumi_subject_id: i64,
    pub subscribed_at: String,
    pub latest_ready_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PlaybackHistoryEntry {
    pub bangumi_subject_id: i64,
    pub bangumi_episode_id: i64,
    pub file_name: Option<String>,
    pub source_fansub_name: Option<String>,
    pub last_played_at: String,
    pub play_count: i64,
}

#[derive(Debug, Clone)]
pub struct CachedBangumiSubjectSummary {
    pub title: String,
    pub title_cn: String,
    pub release_status: String,
}

pub async fn connect_and_migrate(config: &AppConfig) -> anyhow::Result<SqlitePool> {
    if let Some(parent) = config.storage.database_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create database directory {}",
                    parent.to_string_lossy()
                )
            })?;
        }
    }

    fs::create_dir_all(&config.storage.media_root).with_context(|| {
        format!(
            "failed to create media root {}",
            config.storage.media_root.display()
        )
    })?;

    let options = SqliteConnectOptions::new()
        .filename(Path::new(&config.storage.database_path))
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .context("failed to connect to sqlite")?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .context("failed to run migrations")?;

    Ok(pool)
}

pub async fn ensure_bootstrap_admin(pool: &SqlitePool, auth: &AuthConfig) -> Result<(), AppError> {
    let existing = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM admin_accounts")
        .fetch_one(pool)
        .await
        .map_err(|_| AppError::internal("failed to count admin accounts"))?;

    if existing > 0 {
        return Ok(());
    }

    let password_hash = hash_password(&auth.default_admin_password)?;

    sqlx::query(
        "INSERT INTO admin_accounts (username, password_hash, created_at) VALUES (?1, ?2, ?3)",
    )
    .bind(&auth.default_admin_username)
    .bind(password_hash)
    .bind(now_string())
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to create bootstrap admin"))?;

    Ok(())
}

pub async fn touch_device(pool: &SqlitePool, device_id: &str) -> Result<(), AppError> {
    let now = now_string();

    sqlx::query(
        "INSERT INTO devices (id, created_at, last_seen_at)
         VALUES (?1, ?2, ?2)
         ON CONFLICT(id) DO UPDATE SET last_seen_at = excluded.last_seen_at",
    )
    .bind(device_id)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to store device identity"))?;

    Ok(())
}

pub async fn register_user(
    pool: &SqlitePool,
    username: &str,
    password: &str,
    auth: &AuthConfig,
) -> Result<(ViewerIdentity, String), AppError> {
    let password_hash = hash_password(password)?;
    let created_at = now_string();

    let result =
        sqlx::query("INSERT INTO users (username, password_hash, created_at) VALUES (?1, ?2, ?3)")
            .bind(username)
            .bind(password_hash)
            .bind(created_at)
            .execute(pool)
            .await;

    let user_id = match result {
        Ok(result) => result.last_insert_rowid(),
        Err(_) => return Err(AppError::bad_request("username is already in use")),
    };

    let token = create_user_session(pool, user_id, auth.user_session_days).await?;

    Ok((
        ViewerIdentity::User {
            id: user_id,
            username: username.to_owned(),
        },
        token,
    ))
}

pub async fn login_user(
    pool: &SqlitePool,
    username: &str,
    password: &str,
    auth: &AuthConfig,
) -> Result<(ViewerIdentity, String), AppError> {
    let Some(user) = sqlx::query_as::<_, UserRow>(
        "SELECT id, username, password_hash FROM users WHERE username = ?1",
    )
    .bind(username)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::internal("failed to query user"))?
    else {
        return Err(AppError::unauthorized("invalid username or password"));
    };

    if !verify_password(&user.password_hash, password) {
        return Err(AppError::unauthorized("invalid username or password"));
    }

    let token = create_user_session(pool, user.id, auth.user_session_days).await?;

    Ok((
        ViewerIdentity::User {
            id: user.id,
            username: user.username,
        },
        token,
    ))
}

pub async fn login_admin(
    pool: &SqlitePool,
    username: &str,
    password: &str,
    auth: &AuthConfig,
) -> Result<(AdminIdentity, String), AppError> {
    let Some(admin) = sqlx::query_as::<_, AdminRow>(
        "SELECT id, username, password_hash FROM admin_accounts WHERE username = ?1",
    )
    .bind(username)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::internal("failed to query admin account"))?
    else {
        return Err(AppError::unauthorized("invalid admin username or password"));
    };

    if !verify_password(&admin.password_hash, password) {
        return Err(AppError::unauthorized("invalid admin username or password"));
    }

    let token = create_admin_session(pool, admin.id, auth.admin_session_hours).await?;

    Ok((
        AdminIdentity {
            username: admin.username,
        },
        token,
    ))
}

pub async fn user_from_token(
    pool: &SqlitePool,
    token: &str,
) -> Result<Option<ViewerIdentity>, AppError> {
    let row = sqlx::query_as::<_, (i64, String)>(
        "SELECT users.id, users.username
         FROM user_sessions
         INNER JOIN users ON users.id = user_sessions.user_id
         WHERE user_sessions.token = ?1 AND user_sessions.expires_at > ?2",
    )
    .bind(token)
    .bind(now_string())
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::internal("failed to read user session"))?;

    Ok(row.map(|(id, username)| ViewerIdentity::User { id, username }))
}

pub async fn admin_from_token(
    pool: &SqlitePool,
    token: &str,
) -> Result<Option<AdminIdentity>, AppError> {
    let row = sqlx::query_as::<_, (i64, String)>(
        "SELECT admin_accounts.id, admin_accounts.username
         FROM admin_sessions
         INNER JOIN admin_accounts ON admin_accounts.id = admin_sessions.admin_id
         WHERE admin_sessions.token = ?1 AND admin_sessions.expires_at > ?2",
    )
    .bind(token)
    .bind(now_string())
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::internal("failed to read admin session"))?;

    Ok(row.map(|(_, username)| AdminIdentity { username }))
}

pub async fn logout_user(pool: &SqlitePool, token: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM user_sessions WHERE token = ?1")
        .bind(token)
        .execute(pool)
        .await
        .map_err(|_| AppError::internal("failed to delete user session"))?;

    Ok(())
}

pub async fn logout_admin(pool: &SqlitePool, token: &str) -> Result<(), AppError> {
    sqlx::query("DELETE FROM admin_sessions WHERE token = ?1")
        .bind(token)
        .execute(pool)
        .await
        .map_err(|_| AppError::internal("failed to delete admin session"))?;

    Ok(())
}

pub async fn toggle_subscription(
    pool: &SqlitePool,
    viewer: &ViewerIdentity,
    bangumi_subject_id: i64,
) -> Result<(bool, i64), AppError> {
    let now = now_string();

    match viewer {
        ViewerIdentity::Device { id } => {
            let exists = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM device_subscriptions WHERE device_id = ?1 AND bangumi_subject_id = ?2",
            )
            .bind(id)
            .bind(bangumi_subject_id)
            .fetch_one(pool)
            .await
            .map_err(|_| AppError::internal("failed to query device subscriptions"))?;

            if exists > 0 {
                sqlx::query(
                    "DELETE FROM device_subscriptions WHERE device_id = ?1 AND bangumi_subject_id = ?2",
                )
                .bind(id)
                .bind(bangumi_subject_id)
                .execute(pool)
                .await
                .map_err(|_| AppError::internal("failed to remove device subscription"))?;
            } else {
                sqlx::query(
                    "INSERT INTO device_subscriptions (device_id, bangumi_subject_id, created_at) VALUES (?1, ?2, ?3)",
                )
                .bind(id)
                .bind(bangumi_subject_id)
                .bind(now)
                .execute(pool)
                .await
                .map_err(|_| AppError::internal("failed to create device subscription"))?;
            }
        }
        ViewerIdentity::User { id, .. } => {
            let exists = sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM user_subscriptions WHERE user_id = ?1 AND bangumi_subject_id = ?2",
            )
            .bind(id)
            .bind(bangumi_subject_id)
            .fetch_one(pool)
            .await
            .map_err(|_| AppError::internal("failed to query user subscriptions"))?;

            if exists > 0 {
                sqlx::query(
                    "DELETE FROM user_subscriptions WHERE user_id = ?1 AND bangumi_subject_id = ?2",
                )
                .bind(id)
                .bind(bangumi_subject_id)
                .execute(pool)
                .await
                .map_err(|_| AppError::internal("failed to remove user subscription"))?;
            } else {
                sqlx::query(
                    "INSERT INTO user_subscriptions (user_id, bangumi_subject_id, created_at) VALUES (?1, ?2, ?3)",
                )
                .bind(id)
                .bind(bangumi_subject_id)
                .bind(now)
                .execute(pool)
                .await
                .map_err(|_| AppError::internal("failed to create user subscription"))?;
            }
        }
    }

    subscription_state(pool, viewer, bangumi_subject_id).await
}

pub async fn subscription_state(
    pool: &SqlitePool,
    viewer: &ViewerIdentity,
    bangumi_subject_id: i64,
) -> Result<(bool, i64), AppError> {
    let is_subscribed = match viewer {
        ViewerIdentity::Device { id } => {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM device_subscriptions WHERE device_id = ?1 AND bangumi_subject_id = ?2",
            )
            .bind(id)
            .bind(bangumi_subject_id)
            .fetch_one(pool)
            .await
            .map_err(|_| AppError::internal("failed to read device subscription state"))?
                > 0
        }
        ViewerIdentity::User { id, .. } => {
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM user_subscriptions WHERE user_id = ?1 AND bangumi_subject_id = ?2",
            )
            .bind(id)
            .bind(bangumi_subject_id)
            .fetch_one(pool)
            .await
            .map_err(|_| AppError::internal("failed to read user subscription state"))?
                > 0
        }
    };

    Ok((
        is_subscribed,
        total_subscription_count(pool, bangumi_subject_id).await?,
    ))
}

pub async fn list_viewer_subscription_subjects(
    pool: &SqlitePool,
    viewer: &ViewerIdentity,
) -> Result<Vec<ViewerSubscriptionEntry>, AppError> {
    let rows = match viewer {
        ViewerIdentity::Device { id } => sqlx::query_as::<_, ViewerSubscriptionRow>(
            "SELECT
                    device_subscriptions.bangumi_subject_id,
                    device_subscriptions.created_at AS subscribed_at,
                    MAX(media_inventory.updated_at) AS latest_ready_at
                 FROM device_subscriptions
                 LEFT JOIN media_inventory
                    ON media_inventory.bangumi_subject_id = device_subscriptions.bangumi_subject_id
                   AND media_inventory.status = 'ready'
                 WHERE device_subscriptions.device_id = ?1
                 GROUP BY device_subscriptions.bangumi_subject_id, device_subscriptions.created_at",
        )
        .bind(id)
        .fetch_all(pool)
        .await
        .map_err(|_| AppError::internal("failed to list device subscriptions"))?,
        ViewerIdentity::User { id, .. } => sqlx::query_as::<_, ViewerSubscriptionRow>(
            "SELECT
                    user_subscriptions.bangumi_subject_id,
                    user_subscriptions.created_at AS subscribed_at,
                    MAX(media_inventory.updated_at) AS latest_ready_at
                 FROM user_subscriptions
                 LEFT JOIN media_inventory
                    ON media_inventory.bangumi_subject_id = user_subscriptions.bangumi_subject_id
                   AND media_inventory.status = 'ready'
                 WHERE user_subscriptions.user_id = ?1
                 GROUP BY user_subscriptions.bangumi_subject_id, user_subscriptions.created_at",
        )
        .bind(id)
        .fetch_all(pool)
        .await
        .map_err(|_| AppError::internal("failed to list user subscriptions"))?,
    };

    Ok(rows
        .into_iter()
        .map(|row| ViewerSubscriptionEntry {
            bangumi_subject_id: row.bangumi_subject_id,
            subscribed_at: row.subscribed_at,
            latest_ready_at: row.latest_ready_at,
        })
        .collect())
}

pub async fn load_policy(pool: &SqlitePool) -> Result<PolicyDto, AppError> {
    let row = sqlx::query_as::<_, PolicyRow>(
        "SELECT subscription_threshold,
                replacement_window_hours,
                prefer_same_fansub,
                max_concurrent_downloads,
                upload_limit_mb,
                download_limit_mb
         FROM download_policies WHERE id = 1",
    )
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::internal("failed to load download policy"))?;

    Ok(PolicyDto {
        subscription_threshold: row.subscription_threshold,
        replacement_window_hours: row.replacement_window_hours,
        prefer_same_fansub: row.prefer_same_fansub != 0,
        max_concurrent_downloads: row.max_concurrent_downloads.max(1),
        upload_limit_mb: row.upload_limit_mb.max(0),
        download_limit_mb: row.download_limit_mb.max(0),
    })
}

pub async fn update_policy(
    pool: &SqlitePool,
    subscription_threshold: i64,
    replacement_window_hours: i64,
    prefer_same_fansub: bool,
    max_concurrent_downloads: i64,
    upload_limit_mb: i64,
    download_limit_mb: i64,
) -> Result<PolicyDto, AppError> {
    sqlx::query(
        "UPDATE download_policies
         SET subscription_threshold = ?1,
             replacement_window_hours = ?2,
             prefer_same_fansub = ?3,
             max_concurrent_downloads = ?4,
             upload_limit_mb = ?5,
             download_limit_mb = ?6,
             updated_at = ?7
         WHERE id = 1",
    )
    .bind(subscription_threshold)
    .bind(replacement_window_hours)
    .bind(bool_to_int(prefer_same_fansub))
    .bind(max_concurrent_downloads.max(1))
    .bind(upload_limit_mb.max(0))
    .bind(download_limit_mb.max(0))
    .bind(now_string())
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to update download policy"))?;

    load_policy(pool).await
}

pub async fn apply_torrent_runtime_config(
    pool: &SqlitePool,
    max_concurrent_downloads: i64,
    upload_limit_mb: i64,
    download_limit_mb: i64,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE download_policies
         SET max_concurrent_downloads = ?1,
             upload_limit_mb = ?2,
             download_limit_mb = ?3,
             updated_at = ?4
         WHERE id = 1",
    )
    .bind(max_concurrent_downloads.max(1))
    .bind(upload_limit_mb.max(0))
    .bind(download_limit_mb.max(0))
    .bind(now_string())
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to apply torrent runtime config"))?;

    Ok(())
}

pub async fn list_fansub_rules(pool: &SqlitePool) -> Result<Vec<FansubRuleDto>, AppError> {
    let rows = sqlx::query_as::<_, FansubRuleRow>(
        "SELECT id, fansub_name, locale_preference, priority, is_blacklist
         FROM fansub_rules
         ORDER BY is_blacklist DESC, priority DESC, fansub_name ASC",
    )
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::internal("failed to query fansub rules"))?;

    Ok(rows
        .into_iter()
        .map(|row| FansubRuleDto {
            id: row.id,
            fansub_name: row.fansub_name,
            locale_preference: row.locale_preference,
            priority: row.priority,
            is_blacklist: row.is_blacklist != 0,
        })
        .collect())
}

pub async fn add_fansub_rule(
    pool: &SqlitePool,
    fansub_name: &str,
    locale_preference: &str,
    priority: i64,
    is_blacklist: bool,
) -> Result<FansubRuleDto, AppError> {
    let now = now_string();
    let result = sqlx::query(
        "INSERT INTO fansub_rules (fansub_name, locale_preference, priority, is_blacklist, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )
    .bind(fansub_name)
    .bind(locale_preference)
    .bind(priority)
    .bind(bool_to_int(is_blacklist))
    .bind(now)
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to create fansub rule"))?;

    Ok(FansubRuleDto {
        id: result.last_insert_rowid(),
        fansub_name: fansub_name.to_owned(),
        locale_preference: locale_preference.to_owned(),
        priority,
        is_blacklist,
    })
}

pub async fn admin_counts(pool: &SqlitePool) -> Result<AdminCountsDto, AppError> {
    let devices = count(pool, "SELECT COUNT(*) FROM devices").await?;
    let users = count(pool, "SELECT COUNT(*) FROM users").await?;
    let device_subscriptions = count(pool, "SELECT COUNT(*) FROM device_subscriptions").await?;
    let user_subscriptions = count(pool, "SELECT COUNT(*) FROM user_subscriptions").await?;
    let fansub_rules = count(pool, "SELECT COUNT(*) FROM fansub_rules").await?;

    Ok(AdminCountsDto {
        devices,
        users,
        subscriptions: device_subscriptions + user_subscriptions,
        fansub_rules,
    })
}

pub async fn runtime_overview(pool: &SqlitePool) -> Result<RuntimeOverview, AppError> {
    let devices = count(pool, "SELECT COUNT(*) FROM devices").await?;
    let users = count(pool, "SELECT COUNT(*) FROM users").await?;
    let user_sessions = count(pool, "SELECT COUNT(*) FROM user_sessions").await?;
    let admin_sessions = count(pool, "SELECT COUNT(*) FROM admin_sessions").await?;
    let subscriptions = count(pool, "SELECT COUNT(*) FROM device_subscriptions").await?
        + count(pool, "SELECT COUNT(*) FROM user_subscriptions").await?;
    let open_download_jobs = count(
        pool,
        "SELECT COUNT(*) FROM download_jobs WHERE lifecycle IN ('pending', 'queued', 'planning', 'searching', 'staged', 'downloading', 'seeding')",
    )
    .await?;
    let jobs_with_selection = count(
        pool,
        "SELECT COUNT(*) FROM download_jobs WHERE selected_candidate_id IS NOT NULL",
    )
    .await?;
    let running_searches = count(
        pool,
        "SELECT COUNT(*) FROM resource_search_runs WHERE status = 'running'",
    )
    .await?;
    let resource_candidates = count(pool, "SELECT COUNT(*) FROM resource_candidates").await?;
    let active_executions = count(
        pool,
        "SELECT COUNT(*) FROM download_executions WHERE state IN ('staged', 'starting', 'downloading', 'seeding')",
    )
    .await?;
    let downloaded_bytes = sum_i64(
        pool,
        "SELECT COALESCE(SUM(downloaded_bytes), 0) FROM download_executions WHERE state IN ('staged', 'starting', 'downloading', 'seeding')",
    )
    .await?;
    let uploaded_bytes = sum_i64(
        pool,
        "SELECT COALESCE(SUM(uploaded_bytes), 0) FROM download_executions WHERE state IN ('staged', 'starting', 'downloading', 'seeding')",
    )
    .await?;
    let download_rate_bytes = sum_i64(
        pool,
        "SELECT COALESCE(SUM(download_rate_bytes), 0) FROM download_executions WHERE state IN ('staged', 'starting', 'downloading', 'seeding')",
    )
    .await?;
    let upload_rate_bytes = sum_i64(
        pool,
        "SELECT COALESCE(SUM(upload_rate_bytes), 0) FROM download_executions WHERE state IN ('staged', 'starting', 'downloading', 'seeding')",
    )
    .await?;
    let peer_count = sum_i64(
        pool,
        "SELECT COALESCE(SUM(peer_count), 0) FROM download_executions WHERE state IN ('staged', 'starting', 'downloading', 'seeding')",
    )
    .await?;

    Ok(RuntimeOverview {
        devices,
        users,
        active_sessions: user_sessions + admin_sessions,
        subscriptions,
        open_download_jobs,
        jobs_with_selection,
        running_searches,
        resource_candidates,
        active_executions,
        downloaded_bytes,
        uploaded_bytes,
        download_rate_bytes,
        upload_rate_bytes,
        peer_count,
    })
}

pub async fn total_subscription_count(
    pool: &SqlitePool,
    bangumi_subject_id: i64,
) -> Result<i64, AppError> {
    let device_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM device_subscriptions WHERE bangumi_subject_id = ?1",
    )
    .bind(bangumi_subject_id)
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::internal("failed to count device subscriptions"))?;

    let user_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM user_subscriptions WHERE bangumi_subject_id = ?1",
    )
    .bind(bangumi_subject_id)
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::internal("failed to count user subscriptions"))?;

    Ok(device_count + user_count)
}

pub async fn upsert_download_subject(
    pool: &SqlitePool,
    bangumi_subject_id: i64,
    release_status: &str,
    demand_state: &str,
    subscription_count: i64,
    threshold_snapshot: i64,
) -> Result<(), AppError> {
    let now = now_string();

    sqlx::query(
        "INSERT INTO download_subjects (
            bangumi_subject_id,
            release_status,
            demand_state,
            subscription_count,
            threshold_snapshot,
            last_evaluated_at,
            created_at,
            updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?6)
         ON CONFLICT(bangumi_subject_id) DO UPDATE SET
            release_status = excluded.release_status,
            demand_state = excluded.demand_state,
            subscription_count = excluded.subscription_count,
            threshold_snapshot = excluded.threshold_snapshot,
            last_evaluated_at = excluded.last_evaluated_at,
            updated_at = excluded.updated_at",
    )
    .bind(bangumi_subject_id)
    .bind(release_status)
    .bind(demand_state)
    .bind(subscription_count)
    .bind(threshold_snapshot)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to upsert download subject state"))?;

    Ok(())
}

pub async fn find_open_download_job(
    pool: &SqlitePool,
    bangumi_subject_id: i64,
) -> Result<Option<DownloadJobDto>, AppError> {
    let row = sqlx::query_as::<_, DownloadJobRow>(
        "SELECT
            id,
            bangumi_subject_id,
            trigger_kind,
            requested_by,
            release_status,
            season_mode,
            lifecycle,
            subscription_count,
            threshold_snapshot,
            engine_name,
            engine_job_ref,
            notes,
            selected_candidate_id,
            selection_updated_at,
            last_search_run_id,
            search_status,
            created_at,
            updated_at
         FROM download_jobs
         WHERE bangumi_subject_id = ?1
           AND lifecycle IN ('pending', 'queued', 'planning', 'searching', 'staged', 'downloading', 'seeding')
         ORDER BY created_at DESC
         LIMIT 1",
    )
    .bind(bangumi_subject_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::internal("failed to query open download job"))?;

    Ok(row.map(map_download_job))
}

pub async fn download_job_by_id(
    pool: &SqlitePool,
    job_id: i64,
) -> Result<Option<DownloadJobDto>, AppError> {
    let row = sqlx::query_as::<_, DownloadJobRow>(
        "SELECT
            id,
            bangumi_subject_id,
            trigger_kind,
            requested_by,
            release_status,
            season_mode,
            lifecycle,
            subscription_count,
            threshold_snapshot,
            engine_name,
            engine_job_ref,
            notes,
            selected_candidate_id,
            selection_updated_at,
            last_search_run_id,
            search_status,
            created_at,
            updated_at
         FROM download_jobs
         WHERE id = ?1",
    )
    .bind(job_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::internal("failed to read download job"))?;

    Ok(row.map(map_download_job))
}

pub async fn create_download_job(
    pool: &SqlitePool,
    job: NewDownloadJob,
) -> Result<DownloadJobDto, AppError> {
    let now = now_string();

    let result = sqlx::query(
        "INSERT INTO download_jobs (
            bangumi_subject_id,
            trigger_kind,
            requested_by,
            release_status,
            season_mode,
            lifecycle,
            subscription_count,
            threshold_snapshot,
            engine_name,
            engine_job_ref,
            notes,
            created_at,
            updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
    )
    .bind(job.bangumi_subject_id)
    .bind(&job.trigger_kind)
    .bind(&job.requested_by)
    .bind(&job.release_status)
    .bind(&job.season_mode)
    .bind(&job.lifecycle)
    .bind(job.subscription_count)
    .bind(job.threshold_snapshot)
    .bind(&job.engine_name)
    .bind(job.engine_job_ref.as_deref())
    .bind(job.notes.as_deref())
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to create download job"))?;

    Ok(DownloadJobDto {
        id: result.last_insert_rowid(),
        bangumi_subject_id: job.bangumi_subject_id,
        trigger_kind: job.trigger_kind,
        requested_by: job.requested_by,
        release_status: job.release_status,
        season_mode: job.season_mode,
        lifecycle: job.lifecycle,
        subscription_count: job.subscription_count,
        threshold_snapshot: job.threshold_snapshot,
        engine_name: job.engine_name,
        engine_job_ref: job.engine_job_ref,
        notes: job.notes,
        selected_candidate_id: None,
        selection_updated_at: None,
        last_search_run_id: None,
        search_status: "idle".to_owned(),
        created_at: now.clone(),
        updated_at: now,
    })
}

pub async fn mark_download_subject_queued(
    pool: &SqlitePool,
    bangumi_subject_id: i64,
    job_id: i64,
) -> Result<(), AppError> {
    let now = now_string();

    sqlx::query(
        "UPDATE download_subjects
         SET last_queued_job_id = ?2,
             last_triggered_at = ?3,
             updated_at = ?3
         WHERE bangumi_subject_id = ?1",
    )
    .bind(bangumi_subject_id)
    .bind(job_id)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to update download subject queue state"))?;

    Ok(())
}

pub async fn cached_bangumi_subject_summary(
    pool: &SqlitePool,
    bangumi_subject_id: i64,
) -> Result<Option<CachedBangumiSubjectSummary>, AppError> {
    let row = sqlx::query_as::<_, CachedBangumiSubjectSummaryRow>(
        "SELECT
            title,
            title_cn,
            release_status
         FROM bangumi_subject_cache
         WHERE bangumi_subject_id = ?1
         LIMIT 1",
    )
    .bind(bangumi_subject_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::internal("failed to read cached Bangumi subject summary"))?;

    Ok(row.map(|row| CachedBangumiSubjectSummary {
        title: row.title,
        title_cn: row.title_cn,
        release_status: row.release_status,
    }))
}

pub async fn subject_download_status(
    pool: &SqlitePool,
    bangumi_subject_id: i64,
) -> Result<Option<SubjectDownloadStatusDto>, AppError> {
    let subject = sqlx::query_as::<_, DownloadSubjectRow>(
        "SELECT
            bangumi_subject_id,
            release_status,
            demand_state,
            subscription_count,
            threshold_snapshot,
            last_queued_job_id,
            last_evaluated_at
         FROM download_subjects
         WHERE bangumi_subject_id = ?1",
    )
    .bind(bangumi_subject_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::internal("failed to read download subject state"))?;

    let Some(subject) = subject else {
        return Ok(None);
    };

    let job = if let Some(job_id) = subject.last_queued_job_id {
        download_job_by_id(pool, job_id).await?
    } else {
        find_open_download_job(pool, bangumi_subject_id).await?
    };
    let selected_candidate = if let Some(job) = job.as_ref() {
        current_selected_candidate_for_job(pool, job.id).await?
    } else {
        None
    };
    let execution = if let Some(job) = job.as_ref() {
        list_download_executions(pool, job.id)
            .await?
            .into_iter()
            .find(|item| item.state != "replaced")
    } else {
        None
    };
    let (ready_media_count, latest_ready_episode, last_ready_at) =
        sqlx::query_as::<_, (i64, Option<f64>, Option<String>)>(
            "SELECT
                COUNT(*),
                MAX(COALESCE(episode_end_index, episode_index)),
                MAX(updated_at)
             FROM media_inventory
             WHERE bangumi_subject_id = ?1
               AND status = 'ready'",
        )
        .bind(bangumi_subject_id)
        .fetch_one(pool)
        .await
        .map_err(|_| AppError::internal("failed to aggregate subject media readiness"))?;

    Ok(Some(SubjectDownloadStatusDto {
        bangumi_subject_id: subject.bangumi_subject_id,
        release_status: subject.release_status,
        demand_state: subject.demand_state,
        subscription_count: subject.subscription_count,
        threshold_snapshot: subject.threshold_snapshot,
        last_queued_job_id: job
            .as_ref()
            .map(|item| item.id)
            .or(subject.last_queued_job_id),
        job_lifecycle: job.as_ref().map(|item| item.lifecycle.clone()),
        search_status: job.as_ref().map(|item| item.search_status.clone()),
        selected_candidate_id: selected_candidate.as_ref().map(|item| item.id),
        selected_title: selected_candidate.as_ref().map(|item| item.title.clone()),
        execution_id: execution.as_ref().map(|item| item.id),
        execution_state: execution.as_ref().map(|item| item.state.clone()),
        source_title: execution
            .as_ref()
            .map(|item| item.source_title.clone())
            .or_else(|| selected_candidate.as_ref().map(|item| item.title.clone())),
        source_fansub_name: execution
            .as_ref()
            .and_then(|item| item.source_fansub_name.clone())
            .or_else(|| {
                selected_candidate
                    .as_ref()
                    .and_then(|item| item.fansub_name.clone())
            }),
        downloaded_bytes: execution
            .as_ref()
            .map(|item| item.downloaded_bytes)
            .unwrap_or(0),
        total_bytes: execution
            .as_ref()
            .map(|item| item.source_size_bytes.max(item.downloaded_bytes))
            .unwrap_or(0),
        download_rate_bytes: execution
            .as_ref()
            .map(|item| item.download_rate_bytes)
            .unwrap_or(0),
        upload_rate_bytes: execution
            .as_ref()
            .map(|item| item.upload_rate_bytes)
            .unwrap_or(0),
        peer_count: execution.as_ref().map(|item| item.peer_count).unwrap_or(0),
        ready_media_count,
        latest_ready_episode,
        last_ready_at,
        last_evaluated_at: subject.last_evaluated_at,
    }))
}

pub async fn list_download_jobs(
    pool: &SqlitePool,
    limit: usize,
) -> Result<Vec<DownloadJobDto>, AppError> {
    let limit = limit.clamp(1, 100) as i64;
    let rows = sqlx::query_as::<_, DownloadJobRow>(
        "SELECT
            id,
            bangumi_subject_id,
            trigger_kind,
            requested_by,
            release_status,
            season_mode,
            lifecycle,
            subscription_count,
            threshold_snapshot,
            engine_name,
            engine_job_ref,
            notes,
            selected_candidate_id,
            selection_updated_at,
            last_search_run_id,
            search_status,
            created_at,
            updated_at
         FROM download_jobs
         ORDER BY created_at DESC
         LIMIT ?1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::internal("failed to list download jobs"))?;

    Ok(rows.into_iter().map(map_download_job).collect())
}

pub async fn update_download_job_lifecycle(
    pool: &SqlitePool,
    download_job_id: i64,
    lifecycle: &str,
    notes: Option<&str>,
) -> Result<(), AppError> {
    let now = now_string();

    sqlx::query(
        "UPDATE download_jobs
         SET lifecycle = ?2,
             notes = COALESCE(?3, notes),
             updated_at = ?4,
             started_at = CASE
                WHEN ?2 IN ('staged', 'downloading', 'seeding') AND started_at IS NULL THEN ?4
                ELSE started_at
             END,
             completed_at = CASE
                WHEN ?2 IN ('completed', 'failed', 'cancelled', 'replaced') THEN ?4
                ELSE completed_at
             END
         WHERE id = ?1",
    )
    .bind(download_job_id)
    .bind(lifecycle)
    .bind(notes)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to update download job lifecycle"))?;

    Ok(())
}

pub async fn start_resource_search_run(
    pool: &SqlitePool,
    download_job_id: i64,
    bangumi_subject_id: i64,
    strategy: &str,
) -> Result<i64, AppError> {
    let now = now_string();

    let result = sqlx::query(
        "INSERT INTO resource_search_runs (
            download_job_id,
            bangumi_subject_id,
            strategy,
            status,
            created_at
         ) VALUES (?1, ?2, ?3, 'running', ?4)",
    )
    .bind(download_job_id)
    .bind(bangumi_subject_id)
    .bind(strategy)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to create resource search run"))?;

    sqlx::query(
        "UPDATE download_jobs
         SET search_status = 'running',
             updated_at = ?2
         WHERE id = ?1",
    )
    .bind(download_job_id)
    .bind(now_string())
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to mark download job as searching"))?;

    Ok(result.last_insert_rowid())
}

pub async fn finish_resource_search_run(
    pool: &SqlitePool,
    search_run_id: i64,
    download_job_id: i64,
    status: &str,
    candidate_count: i64,
    best_candidate_id: Option<i64>,
    notes: Option<&str>,
) -> Result<(), AppError> {
    let now = now_string();

    sqlx::query(
        "UPDATE resource_search_runs
         SET status = ?2,
             candidate_count = ?3,
             best_candidate_id = ?4,
             notes = ?5,
             completed_at = ?6
         WHERE id = ?1",
    )
    .bind(search_run_id)
    .bind(status)
    .bind(candidate_count)
    .bind(best_candidate_id)
    .bind(notes)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to finish resource search run"))?;

    sqlx::query(
        "UPDATE download_jobs
         SET last_search_run_id = ?2,
             search_status = ?3,
             updated_at = ?4
         WHERE id = ?1",
    )
    .bind(download_job_id)
    .bind(search_run_id)
    .bind(status)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to update download job search state"))?;

    Ok(())
}

pub async fn update_download_job_search_status(
    pool: &SqlitePool,
    download_job_id: i64,
    search_status: &str,
    notes: Option<&str>,
) -> Result<(), AppError> {
    let now = now_string();

    sqlx::query(
        "UPDATE download_jobs
         SET search_status = ?2,
             notes = COALESCE(?3, notes),
             updated_at = ?4
         WHERE id = ?1",
    )
    .bind(download_job_id)
    .bind(search_status)
    .bind(notes)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to update download job search status"))?;

    Ok(())
}

pub async fn create_resource_candidate(
    pool: &SqlitePool,
    candidate: NewResourceCandidate,
) -> Result<ResourceCandidateDto, AppError> {
    let now = now_string();
    let result = sqlx::query(
        "INSERT INTO resource_candidates (
            download_job_id,
            search_run_id,
            bangumi_subject_id,
            slot_key,
            episode_index,
            episode_end_index,
            is_collection,
            provider,
            provider_resource_id,
            title,
            href,
            magnet,
            release_type,
            size_bytes,
            fansub_name,
            publisher_name,
            source_created_at,
            source_fetched_at,
            resolution,
            locale_hint,
            is_raw,
            score,
            rejected_reason,
            discovered_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24)
         ON CONFLICT(download_job_id, provider, provider_resource_id) DO UPDATE SET
            search_run_id = excluded.search_run_id,
            slot_key = excluded.slot_key,
            episode_index = excluded.episode_index,
            episode_end_index = excluded.episode_end_index,
            is_collection = excluded.is_collection,
            title = excluded.title,
            href = excluded.href,
            magnet = excluded.magnet,
            release_type = excluded.release_type,
            size_bytes = excluded.size_bytes,
            fansub_name = excluded.fansub_name,
            publisher_name = excluded.publisher_name,
            source_created_at = excluded.source_created_at,
            source_fetched_at = excluded.source_fetched_at,
            resolution = excluded.resolution,
            locale_hint = excluded.locale_hint,
            is_raw = excluded.is_raw,
            score = excluded.score,
            rejected_reason = excluded.rejected_reason,
            discovered_at = excluded.discovered_at",
    )
    .bind(candidate.download_job_id)
    .bind(candidate.search_run_id)
    .bind(candidate.bangumi_subject_id)
    .bind(&candidate.slot_key)
    .bind(candidate.episode_index)
    .bind(candidate.episode_end_index)
    .bind(bool_to_int(candidate.is_collection))
    .bind(&candidate.provider)
    .bind(&candidate.provider_resource_id)
    .bind(&candidate.title)
    .bind(&candidate.href)
    .bind(&candidate.magnet)
    .bind(&candidate.release_type)
    .bind(candidate.size_bytes)
    .bind(candidate.fansub_name.as_deref())
    .bind(&candidate.publisher_name)
    .bind(&candidate.source_created_at)
    .bind(&candidate.source_fetched_at)
    .bind(candidate.resolution.as_deref())
    .bind(candidate.locale_hint.as_deref())
    .bind(bool_to_int(candidate.is_raw))
    .bind(candidate.score)
    .bind(candidate.rejected_reason.as_deref())
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to create resource candidate"))?;

    let row = sqlx::query_as::<_, ResourceCandidateRow>(
        "SELECT *
         FROM resource_candidates
         WHERE download_job_id = ?1 AND provider = ?2 AND provider_resource_id = ?3",
    )
    .bind(candidate.download_job_id)
    .bind(&candidate.provider)
    .bind(&candidate.provider_resource_id)
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::internal("failed to read resource candidate"))?;

    if result.rows_affected() < 1 {
        return Err(AppError::internal("failed to store resource candidate"));
    }

    Ok(map_resource_candidate(row))
}

pub async fn assign_download_job_candidate(
    pool: &SqlitePool,
    download_job_id: i64,
    candidate_id: Option<i64>,
) -> Result<(), AppError> {
    let now = now_string();

    sqlx::query(
        "UPDATE download_jobs
         SET selected_candidate_id = ?2,
             selection_updated_at = ?3,
             updated_at = ?3
         WHERE id = ?1",
    )
    .bind(download_job_id)
    .bind(candidate_id)
    .bind(now)
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to assign selected resource candidate"))?;

    Ok(())
}

pub async fn current_selected_candidate_for_job(
    pool: &SqlitePool,
    download_job_id: i64,
) -> Result<Option<ResourceCandidateDto>, AppError> {
    let row = sqlx::query_as::<_, ResourceCandidateRow>(
        "SELECT resource_candidates.*
         FROM download_jobs
         INNER JOIN resource_candidates ON resource_candidates.id = download_jobs.selected_candidate_id
         WHERE download_jobs.id = ?1",
    )
    .bind(download_job_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::internal("failed to read selected resource candidate"))?;

    Ok(row.map(map_resource_candidate))
}

pub async fn resource_candidate_by_id(
    pool: &SqlitePool,
    resource_candidate_id: i64,
) -> Result<Option<ResourceCandidateDto>, AppError> {
    let row = sqlx::query_as::<_, ResourceCandidateRow>(
        "SELECT *
         FROM resource_candidates
         WHERE id = ?1",
    )
    .bind(resource_candidate_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::internal("failed to read resource candidate"))?;

    Ok(row.map(map_resource_candidate))
}

pub async fn latest_selected_candidate_for_subject(
    pool: &SqlitePool,
    bangumi_subject_id: i64,
) -> Result<Option<ResourceCandidateDto>, AppError> {
    let row = sqlx::query_as::<_, ResourceCandidateRow>(
        "SELECT resource_candidates.*
         FROM download_jobs
         INNER JOIN resource_candidates ON resource_candidates.id = download_jobs.selected_candidate_id
         WHERE download_jobs.bangumi_subject_id = ?1
         ORDER BY download_jobs.selection_updated_at DESC, download_jobs.created_at DESC
         LIMIT 1",
    )
    .bind(bangumi_subject_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::internal("failed to read latest selected candidate"))?;

    Ok(row.map(map_resource_candidate))
}

pub async fn list_resource_candidates(
    pool: &SqlitePool,
    download_job_id: i64,
) -> Result<Vec<ResourceCandidateDto>, AppError> {
    let rows = sqlx::query_as::<_, ResourceCandidateRow>(
        "SELECT *
         FROM resource_candidates
         WHERE download_job_id = ?1
         ORDER BY rejected_reason IS NOT NULL ASC, score DESC, source_created_at DESC",
    )
    .bind(download_job_id)
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::internal("failed to list resource candidates"))?;

    Ok(rows.into_iter().map(map_resource_candidate).collect())
}

pub async fn find_active_execution_for_job_slot(
    pool: &SqlitePool,
    download_job_id: i64,
    slot_key: &str,
) -> Result<Option<DownloadExecutionDto>, AppError> {
    let row = sqlx::query_as::<_, DownloadExecutionRow>(
        "SELECT *
         FROM download_executions
         WHERE download_job_id = ?1
           AND slot_key = ?2
           AND state IN ('staged', 'starting', 'downloading', 'seeding')
         ORDER BY created_at DESC
         LIMIT 1",
    )
    .bind(download_job_id)
    .bind(slot_key)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::internal("failed to read active download execution by slot"))?;

    Ok(row.map(map_download_execution))
}

pub async fn find_execution_for_job_candidate(
    pool: &SqlitePool,
    download_job_id: i64,
    resource_candidate_id: i64,
) -> Result<Option<DownloadExecutionDto>, AppError> {
    let row = sqlx::query_as::<_, DownloadExecutionRow>(
        "SELECT *
         FROM download_executions
         WHERE download_job_id = ?1 AND resource_candidate_id = ?2
         ORDER BY created_at DESC
         LIMIT 1",
    )
    .bind(download_job_id)
    .bind(resource_candidate_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::internal("failed to read candidate execution"))?;

    Ok(row.map(map_download_execution))
}

pub async fn create_download_execution(
    pool: &SqlitePool,
    execution: NewDownloadExecution,
) -> Result<DownloadExecutionDto, AppError> {
    let now = now_string();
    let result = sqlx::query(
        "INSERT INTO download_executions (
            download_job_id,
            resource_candidate_id,
            bangumi_subject_id,
            slot_key,
            episode_index,
            episode_end_index,
            is_collection,
            engine_name,
            engine_execution_ref,
            execution_role,
            state,
            target_path,
            source_title,
            source_magnet,
            source_size_bytes,
            source_fansub_name,
            downloaded_bytes,
            uploaded_bytes,
            download_rate_bytes,
            upload_rate_bytes,
            peer_count,
            notes,
            created_at,
            updated_at,
            started_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?23,
                   CASE WHEN ?11 IN ('staged', 'starting', 'downloading', 'seeding') THEN ?23 ELSE NULL END)",
    )
    .bind(execution.download_job_id)
    .bind(execution.resource_candidate_id)
    .bind(execution.bangumi_subject_id)
    .bind(&execution.slot_key)
    .bind(execution.episode_index)
    .bind(execution.episode_end_index)
    .bind(bool_to_int(execution.is_collection))
    .bind(&execution.engine_name)
    .bind(execution.engine_execution_ref.as_deref())
    .bind(&execution.execution_role)
    .bind(&execution.state)
    .bind(&execution.target_path)
    .bind(&execution.source_title)
    .bind(&execution.source_magnet)
    .bind(execution.source_size_bytes)
    .bind(execution.source_fansub_name.as_deref())
    .bind(execution.downloaded_bytes)
    .bind(execution.uploaded_bytes)
    .bind(execution.download_rate_bytes)
    .bind(execution.upload_rate_bytes)
    .bind(execution.peer_count)
    .bind(execution.notes.as_deref())
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to create download execution"))?;

    let row = sqlx::query_as::<_, DownloadExecutionRow>(
        "SELECT * FROM download_executions WHERE id = ?1",
    )
    .bind(result.last_insert_rowid())
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::internal("failed to read download execution"))?;

    Ok(map_download_execution(row))
}

#[allow(dead_code)]
pub async fn update_download_execution_metrics(
    pool: &SqlitePool,
    execution_id: i64,
    state: &str,
    downloaded_bytes: i64,
    total_bytes: i64,
    uploaded_bytes: i64,
    download_rate_bytes: i64,
    upload_rate_bytes: i64,
    peer_count: i64,
    notes: Option<&str>,
) -> Result<(), AppError> {
    let now = now_string();

    sqlx::query(
        "UPDATE download_executions
         SET state = ?2,
             downloaded_bytes = ?3,
             source_size_bytes = CASE
                WHEN ?4 > 0 THEN MAX(?4, ?3)
                ELSE MAX(source_size_bytes, ?3)
             END,
             uploaded_bytes = ?5,
             download_rate_bytes = ?6,
             upload_rate_bytes = ?7,
             peer_count = ?8,
             notes = COALESCE(?9, notes),
             updated_at = ?10,
             started_at = CASE
                WHEN ?2 IN ('starting', 'downloading', 'seeding') AND started_at IS NULL THEN ?10
                ELSE started_at
             END,
             completed_at = CASE
                WHEN ?2 IN ('completed', 'seeding') THEN COALESCE(completed_at, ?10)
                ELSE completed_at
             END,
             failed_at = CASE
                WHEN ?2 = 'failed' THEN COALESCE(failed_at, ?10)
                ELSE failed_at
             END
         WHERE id = ?1",
    )
    .bind(execution_id)
    .bind(state)
    .bind(downloaded_bytes)
    .bind(total_bytes)
    .bind(uploaded_bytes)
    .bind(download_rate_bytes)
    .bind(upload_rate_bytes)
    .bind(peer_count)
    .bind(notes)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to update download execution"))?;

    Ok(())
}

pub async fn mark_download_execution_indexed(
    pool: &SqlitePool,
    execution_id: i64,
) -> Result<(), AppError> {
    let now = now_string();

    sqlx::query(
        "UPDATE download_executions
         SET last_indexed_at = ?2,
             updated_at = ?2
         WHERE id = ?1",
    )
    .bind(execution_id)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to mark download execution as indexed"))?;

    Ok(())
}

pub async fn replace_media_inventory_for_execution(
    pool: &SqlitePool,
    execution_id: i64,
    items: &[NewMediaInventoryItem],
) -> Result<(), AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| AppError::internal("failed to start media inventory transaction"))?;

    sqlx::query("DELETE FROM media_inventory WHERE download_execution_id = ?1")
        .bind(execution_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::internal("failed to clear media inventory rows"))?;

    for item in items {
        let now = now_string();
        sqlx::query(
            "INSERT INTO media_inventory (
                bangumi_subject_id,
                download_job_id,
                download_execution_id,
                resource_candidate_id,
                slot_key,
                relative_path,
                absolute_path,
                file_name,
                file_ext,
                size_bytes,
                episode_index,
                episode_end_index,
                is_collection,
                status,
                created_at,
                updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?15)",
        )
        .bind(item.bangumi_subject_id)
        .bind(item.download_job_id)
        .bind(item.download_execution_id)
        .bind(item.resource_candidate_id)
        .bind(&item.slot_key)
        .bind(&item.relative_path)
        .bind(&item.absolute_path)
        .bind(&item.file_name)
        .bind(&item.file_ext)
        .bind(item.size_bytes)
        .bind(item.episode_index)
        .bind(item.episode_end_index)
        .bind(bool_to_int(item.is_collection))
        .bind(&item.status)
        .bind(&now)
        .execute(&mut *tx)
        .await
        .map_err(|_| AppError::internal("failed to insert media inventory row"))?;
    }

    tx.commit()
        .await
        .map_err(|_| AppError::internal("failed to commit media inventory transaction"))?;

    Ok(())
}

pub async fn delete_media_inventory_for_execution(
    pool: &SqlitePool,
    execution_id: i64,
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM media_inventory WHERE download_execution_id = ?1")
        .bind(execution_id)
        .execute(pool)
        .await
        .map_err(|_| AppError::internal("failed to delete media inventory rows"))?;

    Ok(())
}

pub async fn list_active_download_executions(
    pool: &SqlitePool,
    engine_name: &str,
    limit: usize,
) -> Result<Vec<DownloadExecutionDto>, AppError> {
    let limit = limit.clamp(1, 512) as i64;
    let rows = sqlx::query_as::<_, DownloadExecutionRow>(
        "SELECT *
         FROM download_executions
         WHERE engine_name = ?1
           AND state IN ('staged', 'starting', 'downloading', 'seeding', 'completed')
         ORDER BY updated_at DESC, created_at DESC
         LIMIT ?2",
    )
    .bind(engine_name)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::internal("failed to list active download executions"))?;

    Ok(rows.into_iter().map(map_download_execution).collect())
}

pub async fn count_running_download_executions(
    pool: &SqlitePool,
    engine_name: &str,
) -> Result<i64, AppError> {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*)
         FROM download_executions
         WHERE engine_name = ?1
           AND state IN ('starting', 'downloading')",
    )
    .bind(engine_name)
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::internal("failed to count running download executions"))
}

pub async fn list_jobs_ready_for_activation(
    pool: &SqlitePool,
    limit: usize,
) -> Result<Vec<DownloadJobDto>, AppError> {
    let limit = limit.clamp(1, 256) as i64;
    let rows = sqlx::query_as::<_, DownloadJobRow>(
        "SELECT *
         FROM download_jobs
         WHERE selected_candidate_id IS NOT NULL
           AND lifecycle IN ('queued', 'planning', 'searching', 'staged')
         ORDER BY created_at ASC, id ASC
         LIMIT ?1",
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::internal("failed to list jobs ready for activation"))?;

    Ok(rows.into_iter().map(map_download_job).collect())
}

pub async fn mark_download_execution_replaced(
    pool: &SqlitePool,
    execution_id: i64,
    notes: Option<&str>,
) -> Result<(), AppError> {
    let now = now_string();

    sqlx::query(
        "UPDATE download_executions
         SET state = 'replaced',
             notes = COALESCE(?2, notes),
             updated_at = ?3,
             replaced_at = ?3
         WHERE id = ?1",
    )
    .bind(execution_id)
    .bind(notes)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to mark download execution as replaced"))?;

    Ok(())
}

pub async fn list_download_executions(
    pool: &SqlitePool,
    download_job_id: i64,
) -> Result<Vec<DownloadExecutionDto>, AppError> {
    let rows = sqlx::query_as::<_, DownloadExecutionRow>(
        "SELECT *
         FROM download_executions
         WHERE download_job_id = ?1
         ORDER BY created_at DESC",
    )
    .bind(download_job_id)
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::internal("failed to list download executions"))?;

    Ok(rows.into_iter().map(map_download_execution).collect())
}

pub async fn create_download_execution_event(
    pool: &SqlitePool,
    event: NewDownloadExecutionEvent,
) -> Result<DownloadExecutionEventDto, AppError> {
    let now = now_string();
    let result = sqlx::query(
        "INSERT INTO download_execution_events (
            download_execution_id,
            level,
            event_kind,
            message,
            downloaded_bytes,
            uploaded_bytes,
            download_rate_bytes,
            upload_rate_bytes,
            peer_count,
            created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
    )
    .bind(event.download_execution_id)
    .bind(&event.level)
    .bind(&event.event_kind)
    .bind(&event.message)
    .bind(event.downloaded_bytes)
    .bind(event.uploaded_bytes)
    .bind(event.download_rate_bytes)
    .bind(event.upload_rate_bytes)
    .bind(event.peer_count)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to create download execution event"))?;

    let row = sqlx::query_as::<_, DownloadExecutionEventRow>(
        "SELECT
            id,
            download_execution_id,
            level,
            event_kind,
            message,
            downloaded_bytes,
            uploaded_bytes,
            download_rate_bytes,
            upload_rate_bytes,
            peer_count,
            created_at
         FROM download_execution_events
         WHERE id = ?1",
    )
    .bind(result.last_insert_rowid())
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::internal("failed to read download execution event"))?;

    Ok(map_download_execution_event(row))
}

pub async fn list_download_execution_events(
    pool: &SqlitePool,
    download_execution_id: i64,
) -> Result<Vec<DownloadExecutionEventDto>, AppError> {
    let rows = sqlx::query_as::<_, DownloadExecutionEventRow>(
        "SELECT
            id,
            download_execution_id,
            level,
            event_kind,
            message,
            downloaded_bytes,
            uploaded_bytes,
            download_rate_bytes,
            upload_rate_bytes,
            peer_count,
            created_at
         FROM download_execution_events
         WHERE download_execution_id = ?1
         ORDER BY created_at DESC",
    )
    .bind(download_execution_id)
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::internal("failed to list download execution events"))?;

    Ok(rows.into_iter().map(map_download_execution_event).collect())
}

pub async fn list_subject_episode_availability(
    pool: &SqlitePool,
    bangumi_subject_id: i64,
) -> Result<Vec<SubjectEpisodeAvailability>, AppError> {
    let rows = sqlx::query_as::<_, SubjectEpisodeAvailabilityRow>(
        "SELECT
            media_inventory.episode_index,
            media_inventory.episode_end_index,
            media_inventory.is_collection,
            media_inventory.status
         FROM media_inventory
         INNER JOIN download_executions
            ON download_executions.id = media_inventory.download_execution_id
         WHERE media_inventory.bangumi_subject_id = ?1
           AND media_inventory.status IN ('ready', 'partial')
           AND download_executions.state IN ('starting', 'downloading', 'completed', 'seeding')
         ORDER BY CASE media_inventory.status
             WHEN 'ready' THEN 0
             ELSE 1
         END ASC,
         media_inventory.updated_at DESC,
         media_inventory.id DESC",
    )
    .bind(bangumi_subject_id)
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::internal("failed to list subject episode availability"))?;

    Ok(rows
        .into_iter()
        .map(|row| SubjectEpisodeAvailability {
            episode_index: row.episode_index,
            episode_end_index: row.episode_end_index,
            is_collection: row.is_collection != 0,
            status: row.status,
        })
        .collect())
}

pub async fn find_episode_playback_media(
    pool: &SqlitePool,
    bangumi_subject_id: i64,
    episode_number: f64,
) -> Result<Option<ResourceLibraryItemDto>, AppError> {
    let row = sqlx::query_as::<_, ResourceLibraryRow>(
        "SELECT
            media_inventory.id,
            media_inventory.bangumi_subject_id,
            media_inventory.download_job_id,
            media_inventory.download_execution_id,
            media_inventory.resource_candidate_id,
            media_inventory.slot_key,
            download_executions.source_title,
            download_executions.source_fansub_name,
            download_executions.state AS execution_state,
            media_inventory.relative_path,
            media_inventory.absolute_path,
            media_inventory.file_name,
            media_inventory.file_ext,
            media_inventory.size_bytes,
            media_inventory.episode_index,
            media_inventory.episode_end_index,
            media_inventory.is_collection,
            media_inventory.status,
            media_inventory.updated_at
         FROM media_inventory
         INNER JOIN download_executions
            ON download_executions.id = media_inventory.download_execution_id
         WHERE media_inventory.bangumi_subject_id = ?1
           AND media_inventory.status = 'ready'
           AND download_executions.state IN ('completed', 'seeding')
           AND media_inventory.episode_index IS NOT NULL
           AND media_inventory.episode_index <= ?2
           AND COALESCE(media_inventory.episode_end_index, media_inventory.episode_index) >= ?2
         ORDER BY CASE
             WHEN media_inventory.is_collection = 0
              AND COALESCE(media_inventory.episode_end_index, media_inventory.episode_index) = media_inventory.episode_index
             THEN 0
             ELSE 1
         END ASC,
         media_inventory.updated_at DESC,
         media_inventory.id DESC
         LIMIT 1",
    )
    .bind(bangumi_subject_id)
    .bind(episode_number)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::internal("failed to resolve episode playback media"))?;

    Ok(row.map(map_resource_library_item))
}

pub async fn has_partial_episode_media(
    pool: &SqlitePool,
    bangumi_subject_id: i64,
    episode_number: f64,
) -> Result<bool, AppError> {
    let count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*)
         FROM media_inventory
         INNER JOIN download_executions
            ON download_executions.id = media_inventory.download_execution_id
         WHERE media_inventory.bangumi_subject_id = ?1
           AND media_inventory.status = 'partial'
           AND download_executions.state IN ('starting', 'downloading', 'completed', 'seeding')
           AND media_inventory.episode_index IS NOT NULL
           AND media_inventory.episode_index <= ?2
           AND COALESCE(media_inventory.episode_end_index, media_inventory.episode_index) >= ?2",
    )
    .bind(bangumi_subject_id)
    .bind(episode_number)
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::internal("failed to resolve partial episode media"))?;

    Ok(count > 0)
}

pub async fn resource_library_item_by_id(
    pool: &SqlitePool,
    media_inventory_id: i64,
) -> Result<Option<ResourceLibraryItemDto>, AppError> {
    let row = sqlx::query_as::<_, ResourceLibraryRow>(
        "SELECT
            media_inventory.id,
            media_inventory.bangumi_subject_id,
            media_inventory.download_job_id,
            media_inventory.download_execution_id,
            media_inventory.resource_candidate_id,
            media_inventory.slot_key,
            download_executions.source_title,
            download_executions.source_fansub_name,
            download_executions.state AS execution_state,
            media_inventory.relative_path,
            media_inventory.absolute_path,
            media_inventory.file_name,
            media_inventory.file_ext,
            media_inventory.size_bytes,
            media_inventory.episode_index,
            media_inventory.episode_end_index,
            media_inventory.is_collection,
            media_inventory.status,
            media_inventory.updated_at
         FROM media_inventory
         INNER JOIN download_executions
            ON download_executions.id = media_inventory.download_execution_id
         WHERE media_inventory.id = ?1
         LIMIT 1",
    )
    .bind(media_inventory_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::internal("failed to read media inventory item"))?;

    Ok(row.map(map_resource_library_item))
}

pub async fn list_resource_library_items(
    pool: &SqlitePool,
    keyword: Option<&str>,
    limit: usize,
    offset: usize,
) -> Result<(usize, i64, Vec<ResourceLibraryItemDto>), AppError> {
    let limit = limit.clamp(1, 100) as i64;
    let offset = offset.max(0) as i64;
    let keyword = keyword
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("%{value}%"));

    let total = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*)
         FROM media_inventory
         INNER JOIN download_executions ON download_executions.id = media_inventory.download_execution_id
         WHERE media_inventory.status = 'ready'
           AND (?1 IS NULL
                OR media_inventory.file_name LIKE ?1
                OR download_executions.source_title LIKE ?1
                OR CAST(media_inventory.bangumi_subject_id AS TEXT) LIKE ?1)",
    )
    .bind(keyword.as_deref())
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::internal("failed to count resource library rows"))?;

    let total_size_bytes = sqlx::query_scalar::<_, Option<i64>>(
        "SELECT SUM(media_inventory.size_bytes)
         FROM media_inventory
         INNER JOIN download_executions ON download_executions.id = media_inventory.download_execution_id
         WHERE media_inventory.status = 'ready'
           AND (?1 IS NULL
                OR media_inventory.file_name LIKE ?1
                OR download_executions.source_title LIKE ?1
                OR CAST(media_inventory.bangumi_subject_id AS TEXT) LIKE ?1)",
    )
    .bind(keyword.as_deref())
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::internal("failed to sum resource library size"))?
    .unwrap_or(0);

    let rows = sqlx::query_as::<_, ResourceLibraryRow>(
        "SELECT
            media_inventory.id,
            media_inventory.bangumi_subject_id,
            media_inventory.download_job_id,
            media_inventory.download_execution_id,
            media_inventory.resource_candidate_id,
            media_inventory.slot_key,
            download_executions.source_title,
            download_executions.source_fansub_name,
            download_executions.state AS execution_state,
            media_inventory.relative_path,
            media_inventory.absolute_path,
            media_inventory.file_name,
            media_inventory.file_ext,
            media_inventory.size_bytes,
            media_inventory.episode_index,
            media_inventory.episode_end_index,
            media_inventory.is_collection,
            media_inventory.status,
            media_inventory.updated_at
         FROM media_inventory
         INNER JOIN download_executions ON download_executions.id = media_inventory.download_execution_id
         WHERE media_inventory.status = 'ready'
           AND (?1 IS NULL
                OR media_inventory.file_name LIKE ?1
                OR download_executions.source_title LIKE ?1
                OR CAST(media_inventory.bangumi_subject_id AS TEXT) LIKE ?1)
         ORDER BY media_inventory.updated_at DESC, media_inventory.id DESC
         LIMIT ?2 OFFSET ?3",
    )
    .bind(keyword.as_deref())
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::internal("failed to list resource library rows"))?;

    Ok((
        total.max(0) as usize,
        total_size_bytes.max(0),
        rows.into_iter().map(map_resource_library_item).collect(),
    ))
}

pub async fn record_playback_history(
    pool: &SqlitePool,
    viewer: &ViewerIdentity,
    bangumi_subject_id: i64,
    bangumi_episode_id: i64,
    media_inventory_id: i64,
) -> Result<(), AppError> {
    let (viewer_kind, viewer_key) = viewer_history_identity(viewer);
    let now = now_string();

    sqlx::query(
        "INSERT INTO playback_history (
            viewer_kind,
            viewer_key,
            bangumi_subject_id,
            bangumi_episode_id,
            media_inventory_id,
            last_played_at,
            created_at,
            play_count
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, 1)
         ON CONFLICT(viewer_kind, viewer_key, bangumi_episode_id) DO UPDATE SET
            bangumi_subject_id = excluded.bangumi_subject_id,
            media_inventory_id = excluded.media_inventory_id,
            last_played_at = excluded.last_played_at,
            play_count = playback_history.play_count + 1",
    )
    .bind(&viewer_kind)
    .bind(&viewer_key)
    .bind(bangumi_subject_id)
    .bind(bangumi_episode_id)
    .bind(media_inventory_id)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to record playback history"))?;

    Ok(())
}

pub async fn list_viewer_playback_history(
    pool: &SqlitePool,
    viewer: &ViewerIdentity,
    limit: usize,
    offset: usize,
) -> Result<(usize, Vec<PlaybackHistoryEntry>), AppError> {
    let limit = limit.clamp(1, 100) as i64;
    let offset = offset.max(0) as i64;
    let (viewer_kind, viewer_key) = viewer_history_identity(viewer);

    let total = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*)
         FROM playback_history
         WHERE viewer_kind = ?1
           AND viewer_key = ?2",
    )
    .bind(&viewer_kind)
    .bind(&viewer_key)
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::internal("failed to count playback history rows"))?;

    let rows = sqlx::query_as::<_, PlaybackHistoryRow>(
        "SELECT
            playback_history.bangumi_subject_id,
            playback_history.bangumi_episode_id,
            media_inventory.file_name,
            download_executions.source_fansub_name,
            playback_history.last_played_at,
            playback_history.play_count
         FROM playback_history
         LEFT JOIN media_inventory ON media_inventory.id = playback_history.media_inventory_id
         LEFT JOIN download_executions ON download_executions.id = media_inventory.download_execution_id
         WHERE playback_history.viewer_kind = ?1
           AND playback_history.viewer_key = ?2
         ORDER BY playback_history.last_played_at DESC, playback_history.bangumi_episode_id DESC
         LIMIT ?3 OFFSET ?4",
    )
    .bind(viewer_kind)
    .bind(viewer_key)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(|_| AppError::internal("failed to list playback history rows"))?;

    Ok((
        total.max(0) as usize,
        rows.into_iter()
            .map(|row| PlaybackHistoryEntry {
                bangumi_subject_id: row.bangumi_subject_id,
                bangumi_episode_id: row.bangumi_episode_id,
                file_name: row.file_name,
                source_fansub_name: row.source_fansub_name,
                last_played_at: row.last_played_at,
                play_count: row.play_count,
            })
            .collect(),
    ))
}

async fn count(pool: &SqlitePool, query: &str) -> Result<i64, AppError> {
    sqlx::query_scalar::<_, i64>(query)
        .fetch_one(pool)
        .await
        .map_err(|_| AppError::internal("failed to count rows"))
}

async fn sum_i64(pool: &SqlitePool, query: &str) -> Result<i64, AppError> {
    sqlx::query_scalar::<_, i64>(query)
        .fetch_one(pool)
        .await
        .map_err(|_| AppError::internal("failed to aggregate rows"))
}

fn viewer_history_identity(viewer: &ViewerIdentity) -> (&'static str, String) {
    match viewer {
        ViewerIdentity::Device { id } => ("device", id.clone()),
        ViewerIdentity::User { id, .. } => ("user", id.to_string()),
    }
}

async fn create_user_session(
    pool: &SqlitePool,
    user_id: i64,
    days: i64,
) -> Result<String, AppError> {
    let token = generate_token();
    let created_at = Utc::now();
    let expires_at = created_at + Duration::days(days);

    sqlx::query(
        "INSERT INTO user_sessions (token, user_id, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(&token)
    .bind(user_id)
    .bind(created_at.to_rfc3339())
    .bind(expires_at.to_rfc3339())
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to create user session"))?;

    Ok(token)
}

async fn create_admin_session(
    pool: &SqlitePool,
    admin_id: i64,
    hours: i64,
) -> Result<String, AppError> {
    let token = generate_token();
    let created_at = Utc::now();
    let expires_at = created_at + Duration::hours(hours);

    sqlx::query(
        "INSERT INTO admin_sessions (token, admin_id, created_at, expires_at) VALUES (?1, ?2, ?3, ?4)",
    )
    .bind(&token)
    .bind(admin_id)
    .bind(created_at.to_rfc3339())
    .bind(expires_at.to_rfc3339())
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to create admin session"))?;

    Ok(token)
}

fn now_string() -> String {
    Utc::now().to_rfc3339()
}

fn bool_to_int(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

fn map_download_job(row: DownloadJobRow) -> DownloadJobDto {
    DownloadJobDto {
        id: row.id,
        bangumi_subject_id: row.bangumi_subject_id,
        trigger_kind: row.trigger_kind,
        requested_by: row.requested_by,
        release_status: row.release_status,
        season_mode: row.season_mode,
        lifecycle: row.lifecycle,
        subscription_count: row.subscription_count,
        threshold_snapshot: row.threshold_snapshot,
        engine_name: row.engine_name,
        engine_job_ref: row.engine_job_ref,
        notes: row.notes,
        selected_candidate_id: row.selected_candidate_id,
        selection_updated_at: row.selection_updated_at,
        last_search_run_id: row.last_search_run_id,
        search_status: row.search_status,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn map_resource_candidate(row: ResourceCandidateRow) -> ResourceCandidateDto {
    ResourceCandidateDto {
        id: row.id,
        download_job_id: row.download_job_id,
        search_run_id: row.search_run_id,
        bangumi_subject_id: row.bangumi_subject_id,
        slot_key: row.slot_key,
        episode_index: row.episode_index,
        episode_end_index: row.episode_end_index,
        is_collection: row.is_collection != 0,
        provider: row.provider,
        provider_resource_id: row.provider_resource_id,
        title: row.title,
        href: row.href,
        magnet: row.magnet,
        release_type: row.release_type,
        size_bytes: row.size_bytes,
        fansub_name: row.fansub_name,
        publisher_name: row.publisher_name,
        source_created_at: row.source_created_at,
        source_fetched_at: row.source_fetched_at,
        resolution: row.resolution,
        locale_hint: row.locale_hint,
        is_raw: row.is_raw != 0,
        score: row.score,
        rejected_reason: row.rejected_reason,
        discovered_at: row.discovered_at,
    }
}

fn map_download_execution(row: DownloadExecutionRow) -> DownloadExecutionDto {
    DownloadExecutionDto {
        id: row.id,
        download_job_id: row.download_job_id,
        resource_candidate_id: row.resource_candidate_id,
        bangumi_subject_id: row.bangumi_subject_id,
        slot_key: row.slot_key,
        episode_index: row.episode_index,
        episode_end_index: row.episode_end_index,
        is_collection: row.is_collection != 0,
        engine_name: row.engine_name,
        engine_execution_ref: row.engine_execution_ref,
        execution_role: row.execution_role,
        state: row.state,
        target_path: row.target_path,
        source_title: row.source_title,
        source_magnet: row.source_magnet,
        source_size_bytes: row.source_size_bytes,
        source_fansub_name: row.source_fansub_name,
        downloaded_bytes: row.downloaded_bytes,
        uploaded_bytes: row.uploaded_bytes,
        download_rate_bytes: row.download_rate_bytes,
        upload_rate_bytes: row.upload_rate_bytes,
        peer_count: row.peer_count,
        notes: row.notes,
        created_at: row.created_at,
        updated_at: row.updated_at,
        started_at: row.started_at,
        completed_at: row.completed_at,
        replaced_at: row.replaced_at,
        failed_at: row.failed_at,
        last_indexed_at: row.last_indexed_at,
    }
}

fn map_download_execution_event(row: DownloadExecutionEventRow) -> DownloadExecutionEventDto {
    DownloadExecutionEventDto {
        id: row.id,
        download_execution_id: row.download_execution_id,
        level: row.level,
        event_kind: row.event_kind,
        message: row.message,
        downloaded_bytes: row.downloaded_bytes,
        uploaded_bytes: row.uploaded_bytes,
        download_rate_bytes: row.download_rate_bytes,
        upload_rate_bytes: row.upload_rate_bytes,
        peer_count: row.peer_count,
        created_at: row.created_at,
    }
}

fn map_resource_library_item(row: ResourceLibraryRow) -> ResourceLibraryItemDto {
    ResourceLibraryItemDto {
        id: row.id,
        bangumi_subject_id: row.bangumi_subject_id,
        download_job_id: row.download_job_id,
        download_execution_id: row.download_execution_id,
        resource_candidate_id: row.resource_candidate_id,
        slot_key: row.slot_key,
        source_title: row.source_title,
        source_fansub_name: row.source_fansub_name,
        execution_state: row.execution_state,
        relative_path: row.relative_path,
        absolute_path: row.absolute_path,
        file_name: row.file_name,
        file_ext: row.file_ext,
        size_bytes: row.size_bytes,
        episode_index: row.episode_index,
        episode_end_index: row.episode_end_index,
        is_collection: row.is_collection != 0,
        status: row.status,
        updated_at: row.updated_at,
    }
}
