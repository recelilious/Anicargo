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
        AdminCountsDto, AppError, DownloadJobDto, FansubRuleDto, PolicyDto, ResourceCandidateDto,
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

pub async fn load_policy(pool: &SqlitePool) -> Result<PolicyDto, AppError> {
    let row = sqlx::query_as::<_, PolicyRow>(
        "SELECT subscription_threshold, replacement_window_hours, prefer_same_fansub
         FROM download_policies WHERE id = 1",
    )
    .fetch_one(pool)
    .await
    .map_err(|_| AppError::internal("failed to load download policy"))?;

    Ok(PolicyDto {
        subscription_threshold: row.subscription_threshold,
        replacement_window_hours: row.replacement_window_hours,
        prefer_same_fansub: row.prefer_same_fansub != 0,
    })
}

pub async fn update_policy(
    pool: &SqlitePool,
    subscription_threshold: i64,
    replacement_window_hours: i64,
    prefer_same_fansub: bool,
) -> Result<PolicyDto, AppError> {
    sqlx::query(
        "UPDATE download_policies
         SET subscription_threshold = ?1,
             replacement_window_hours = ?2,
             prefer_same_fansub = ?3,
             updated_at = ?4
         WHERE id = 1",
    )
    .bind(subscription_threshold)
    .bind(replacement_window_hours)
    .bind(bool_to_int(prefer_same_fansub))
    .bind(now_string())
    .execute(pool)
    .await
    .map_err(|_| AppError::internal("failed to update download policy"))?;

    load_policy(pool).await
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
           AND lifecycle IN ('pending', 'queued', 'planning', 'searching', 'downloading', 'seeding')
         ORDER BY created_at DESC
         LIMIT 1",
    )
    .bind(bangumi_subject_id)
    .fetch_optional(pool)
    .await
    .map_err(|_| AppError::internal("failed to query open download job"))?;

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
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
         ON CONFLICT(download_job_id, provider, provider_resource_id) DO UPDATE SET
            search_run_id = excluded.search_run_id,
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
        "SELECT
            id,
            download_job_id,
            search_run_id,
            bangumi_subject_id,
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
        "SELECT
            resource_candidates.id,
            resource_candidates.download_job_id,
            resource_candidates.search_run_id,
            resource_candidates.bangumi_subject_id,
            resource_candidates.provider,
            resource_candidates.provider_resource_id,
            resource_candidates.title,
            resource_candidates.href,
            resource_candidates.magnet,
            resource_candidates.release_type,
            resource_candidates.size_bytes,
            resource_candidates.fansub_name,
            resource_candidates.publisher_name,
            resource_candidates.source_created_at,
            resource_candidates.source_fetched_at,
            resource_candidates.resolution,
            resource_candidates.locale_hint,
            resource_candidates.is_raw,
            resource_candidates.score,
            resource_candidates.rejected_reason,
            resource_candidates.discovered_at
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

pub async fn latest_selected_candidate_for_subject(
    pool: &SqlitePool,
    bangumi_subject_id: i64,
) -> Result<Option<ResourceCandidateDto>, AppError> {
    let row = sqlx::query_as::<_, ResourceCandidateRow>(
        "SELECT
            resource_candidates.id,
            resource_candidates.download_job_id,
            resource_candidates.search_run_id,
            resource_candidates.bangumi_subject_id,
            resource_candidates.provider,
            resource_candidates.provider_resource_id,
            resource_candidates.title,
            resource_candidates.href,
            resource_candidates.magnet,
            resource_candidates.release_type,
            resource_candidates.size_bytes,
            resource_candidates.fansub_name,
            resource_candidates.publisher_name,
            resource_candidates.source_created_at,
            resource_candidates.source_fetched_at,
            resource_candidates.resolution,
            resource_candidates.locale_hint,
            resource_candidates.is_raw,
            resource_candidates.score,
            resource_candidates.rejected_reason,
            resource_candidates.discovered_at
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
        "SELECT
            id,
            download_job_id,
            search_run_id,
            bangumi_subject_id,
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

async fn count(pool: &SqlitePool, query: &str) -> Result<i64, AppError> {
    sqlx::query_scalar::<_, i64>(query)
        .fetch_one(pool)
        .await
        .map_err(|_| AppError::internal("failed to count rows"))
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
