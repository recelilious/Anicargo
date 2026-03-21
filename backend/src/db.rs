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
    types::{AdminCountsDto, AppError, FansubRuleDto, PolicyDto},
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

    let result = sqlx::query("INSERT INTO users (username, password_hash, created_at) VALUES (?1, ?2, ?3)")
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
            username: username.to_owned()
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
    .map_err(|_| AppError::internal("failed to query user"))? else {
        return Err(AppError::unauthorized("invalid username or password"));
    };

    if !verify_password(&user.password_hash, password) {
        return Err(AppError::unauthorized("invalid username or password"));
    }

    let token = create_user_session(pool, user.id, auth.user_session_days).await?;

    Ok((
        ViewerIdentity::User {
            id: user.id,
            username: user.username
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
    .map_err(|_| AppError::internal("failed to query admin account"))? else {
        return Err(AppError::unauthorized("invalid admin username or password"));
    };

    if !verify_password(&admin.password_hash, password) {
        return Err(AppError::unauthorized("invalid admin username or password"));
    }

    let token = create_admin_session(pool, admin.id, auth.admin_session_hours).await?;

    Ok((
        AdminIdentity {
            username: admin.username
        },
        token,
    ))
}

pub async fn user_from_token(pool: &SqlitePool, token: &str) -> Result<Option<ViewerIdentity>, AppError> {
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

    Ok(row.map(|(id, username)| ViewerIdentity::User {
        id,
        username
    }))
}

pub async fn admin_from_token(pool: &SqlitePool, token: &str) -> Result<Option<AdminIdentity>, AppError> {
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

    Ok((is_subscribed, device_count + user_count))
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

async fn count(pool: &SqlitePool, query: &str) -> Result<i64, AppError> {
    sqlx::query_scalar::<_, i64>(query)
        .fetch_one(pool)
        .await
        .map_err(|_| AppError::internal("failed to count rows"))
}

async fn create_user_session(pool: &SqlitePool, user_id: i64, days: i64) -> Result<String, AppError> {
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

async fn create_admin_session(pool: &SqlitePool, admin_id: i64, hours: i64) -> Result<String, AppError> {
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
