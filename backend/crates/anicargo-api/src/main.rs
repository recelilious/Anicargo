use anicargo_bangumi::BangumiClient;
use anicargo_config::{init_logging, split_config_args, AppConfig};
use anicargo_library::{
    auto_match_all, cleanup_jobs, clear_match, complete_job, enqueue_job, fail_job, fetch_next_job,
    get_candidates, get_job_status, get_match, init_library, list_media_entries, requeue_stuck_jobs,
    scan_and_index, set_manual_match, AutoMatchOptions, Job, JobStatus, MatchCandidate, MediaMatch,
};
use anicargo_media::{ensure_hls, find_entry_by_id, MediaConfig, MediaError, MediaEntry};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::{header, HeaderMap, Request, StatusCode};
use axum::middleware;
use axum::response::{IntoResponse, Response};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{delete, get, post};
use axum::Json;
use axum::Router;
use axum::Extension;
use async_stream::stream;
use futures::Stream;
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::env;
use std::fmt;
use std::fs;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path as StdPath, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::fs::File;
use tokio::sync::{Mutex, Semaphore};
use tokio::time::sleep;
use tokio_util::io::ReaderStream;
use tower::limit::ConcurrencyLimitLayer;
use tracing::info;

#[derive(Clone)]
struct AppState {
    config: Arc<MediaConfig>,
    db: PgPool,
    auth: Arc<AuthConfig>,
    bangumi: Arc<BangumiClient>,
    scan_limit: Arc<Semaphore>,
    hls_limit: Arc<Semaphore>,
    hls_locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
    rate_limits: Arc<Mutex<HashMap<String, RateLimitState>>>,
    rate_limit_user_per_minute: u32,
    rate_limit_ip_per_minute: u32,
    rate_limit_allow_users: HashSet<String>,
    rate_limit_allow_ips: HashSet<IpAddr>,
    rate_limit_block_users: HashSet<String>,
    rate_limit_block_ips: HashSet<IpAddr>,
    job_poll_interval_ms: u64,
    job_retention_hours: u64,
    job_running_timeout_secs: u64,
    job_max_attempts: u32,
}

#[derive(Debug, Clone)]
struct AuthConfig {
    jwt_secret: String,
    token_ttl: Duration,
    admin_user: String,
    admin_password: String,
    invite_code: String,
}

#[derive(Debug)]
struct RateLimitState {
    window_start: Instant,
    count: u32,
    last_seen: Instant,
}

#[derive(Debug, Serialize)]
struct StreamResponse {
    id: String,
    playlist_url: String,
}

#[derive(Debug, Serialize)]
struct LoginResponse {
    token: String,
    user_id: String,
    role: UserRole,
    expires_in: u64,
}

#[derive(Debug, Serialize)]
struct CreateUserResponse {
    user_id: String,
    role: UserRole,
}

#[derive(Debug, Serialize)]
struct MatchStatusResponse {
    current: Option<MediaMatch>,
}

#[derive(Debug, Serialize)]
struct MatchCandidatesResponse {
    candidates: Vec<MatchCandidate>,
}

#[derive(Debug, Serialize)]
struct JobIdResponse {
    job_id: i64,
}

#[derive(Debug, Serialize)]
struct JobStatusResponse {
    job: JobStatus,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Debug, Deserialize)]
struct LoginRequest {
    user_id: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct CreateUserRequest {
    user_id: String,
    password: String,
    invite_code: String,
}

#[derive(Debug, Deserialize)]
struct ManualMatchRequest {
    subject_id: i64,
    episode_id: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct AutoMatchJobRequest {
    limit: Option<u32>,
    min_candidate_score: Option<f32>,
    min_confidence: Option<f32>,
}

#[derive(Debug, Deserialize)]
struct TokenQuery {
    token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LibraryQuery {
    token: Option<String>,
    refresh: Option<bool>,
}

#[derive(Debug, Serialize)]
struct StreamPendingResponse {
    status: String,
    job_id: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum UserRole {
    Admin,
    User,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    role: UserRole,
    exp: u64,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ApiError {}

impl From<MediaError> for ApiError {
    fn from(err: MediaError) -> Self {
        match err {
            MediaError::NotFound(message) => ApiError {
                status: StatusCode::NOT_FOUND,
                message,
            },
            MediaError::MissingMediaDir => ApiError {
                status: StatusCode::BAD_REQUEST,
                message: err.to_string(),
            },
            MediaError::InvalidMediaDir(_) | MediaError::InvalidConfig(_) => ApiError {
                status: StatusCode::BAD_REQUEST,
                message: err.to_string(),
            },
            MediaError::Io(_) => ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: err.to_string(),
            },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(ErrorResponse {
            error: self.message,
        });
        (self.status, body).into_response()
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (config_path, _args) = split_config_args(env::args().skip(1))?;
    let app_config = AppConfig::load(config_path)?;
    let _log_guard = init_logging(&app_config.logging)?;

    let media_dir = app_config.media.require_media_dir()?;
    let media_config = MediaConfig {
        media_dir,
        cache_dir: app_config.media.cache_dir.clone(),
        ffmpeg_path: app_config.hls.ffmpeg_path.clone(),
        hls_segment_secs: app_config.hls.segment_secs,
        hls_playlist_len: app_config.hls.playlist_len,
        hls_lock_timeout_secs: app_config.hls.lock_timeout_secs,
        transcode: app_config.hls.transcode,
    };
    let hls_root = media_config.hls_root();
    fs::create_dir_all(&hls_root)?;

    let auth = AuthConfig {
        jwt_secret: app_config.auth.jwt_secret.clone(),
        token_ttl: Duration::from_secs(app_config.auth.token_ttl_secs),
        admin_user: app_config.auth.admin_user.clone(),
        admin_password: app_config.auth.admin_password.clone(),
        invite_code: app_config.auth.invite_code.clone(),
    };
    let bangumi = BangumiClient::new(
        app_config.bangumi.access_token.clone(),
        app_config.bangumi.user_agent.clone(),
    )?;
    let db_url = app_config.db.require_database_url()?;
    let db = connect_db(&db_url, app_config.db.max_connections).await?;
    init_db(&db).await?;
    ensure_admin(&db, &auth).await?;

    let allow_ips = parse_ip_set(&app_config.server.rate_limit_allow_ips)?;
    let block_ips = parse_ip_set(&app_config.server.rate_limit_block_ips)?;
    let rate_limit_enabled = app_config.server.rate_limit_user_per_minute > 0
        || app_config.server.rate_limit_ip_per_minute > 0
        || !app_config.server.rate_limit_allow_users.is_empty()
        || !app_config.server.rate_limit_allow_ips.is_empty()
        || !app_config.server.rate_limit_block_users.is_empty()
        || !app_config.server.rate_limit_block_ips.is_empty();

    let state = Arc::new(AppState {
        config: Arc::new(media_config),
        db,
        auth: Arc::new(auth),
        bangumi: Arc::new(bangumi),
        scan_limit: Arc::new(Semaphore::new(
            app_config.server.max_scan_concurrency as usize,
        )),
        hls_limit: Arc::new(Semaphore::new(
            app_config.server.max_hls_concurrency as usize,
        )),
        hls_locks: Arc::new(Mutex::new(HashMap::new())),
        rate_limits: Arc::new(Mutex::new(HashMap::new())),
        rate_limit_user_per_minute: app_config.server.rate_limit_user_per_minute,
        rate_limit_ip_per_minute: app_config.server.rate_limit_ip_per_minute,
        rate_limit_allow_users: to_string_set(&app_config.server.rate_limit_allow_users),
        rate_limit_allow_ips: allow_ips,
        rate_limit_block_users: to_string_set(&app_config.server.rate_limit_block_users),
        rate_limit_block_ips: block_ips,
        job_poll_interval_ms: app_config.server.job_poll_interval_ms,
        job_retention_hours: app_config.server.job_retention_hours,
        job_running_timeout_secs: app_config.server.job_running_timeout_secs,
        job_max_attempts: app_config.server.job_max_attempts,
    });

    let mut app = Router::new()
        .route("/api/library", get(library_handler))
        .route("/api/stream/:id", get(stream_handler))
        .route("/api/auth/login", post(login_handler))
        .route("/api/users", post(create_user_handler))
        .route("/api/users/:id", delete(delete_user_handler))
        .route("/api/matches/auto", post(auto_match_handler))
        .route("/api/matches/:id", get(match_status_handler).post(manual_match_handler).delete(clear_match_handler))
        .route(
            "/api/matches/:id/candidates",
            get(match_candidates_handler),
        )
        .route("/api/jobs/index", post(enqueue_index_job_handler))
        .route("/api/jobs/auto-match", post(enqueue_auto_match_job_handler))
        .route("/api/jobs/hls/:id", post(enqueue_hls_job_handler))
        .route("/api/jobs/:id", get(job_status_handler))
        .route("/api/jobs/:id/stream", get(job_status_stream_handler))
        .route("/hls/:token/:id/:file", get(hls_file_handler_with_token))
        .route("/hls/:id/:file", get(hls_file_handler));

    app = apply_limits(app, &app_config.server);
    app = app.layer(Extension(state.clone()));
    if rate_limit_enabled {
        app = app.layer(middleware::from_fn_with_state(
            state.clone(),
            rate_limit_middleware,
        ));
    }

    spawn_job_workers(
        state.clone(),
        app_config.server.job_workers,
        Duration::from_millis(app_config.server.job_poll_interval_ms),
    );
    if app_config.server.job_retention_hours > 0
        || app_config.server.job_running_timeout_secs > 0
    {
        spawn_job_cleanup(
            state.clone(),
            Duration::from_secs(app_config.server.job_cleanup_interval_secs),
        );
    }

    let bind_addr = app_config.server.bind.clone();
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;
    info!("anicargo-api listening on {}", bind_addr);
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

async fn library_handler(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<LibraryQuery>,
) -> Result<Json<Vec<MediaEntry>>, ApiError> {
    let token_query = TokenQuery {
        token: query.token.clone(),
    };
    let auth = require_auth(&state, &headers, &token_query, None).await?;
    if query.refresh.unwrap_or(false) {
        if auth.role != UserRole::Admin {
            return Err(ApiError {
                status: StatusCode::FORBIDDEN,
                message: "forbidden".to_string(),
            });
        }
        let _ = enqueue_job(
            &state.db,
            "index",
            json!({"mode":"incremental"}),
            state.job_max_attempts,
            Some("index"),
        )
        .await;
    }
    let entries = list_media_entries(&state.db).await.map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("library lookup failed: {}", err),
    })?;
    info!(count = entries.len(), "library scan completed");
    Ok(Json(entries))
}

async fn stream_handler(
    Path(id): Path<String>,
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<Response, ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    let playlist_path = state.config.hls_root().join(&id).join("index.m3u8");
    if playlist_path.exists() {
        info!(user_id = %auth.user_id, media_id = %id, "stream prepared");
        let playlist_url = format!("/hls/{}/{}/index.m3u8", auth.token, id);
        return Ok(Json(StreamResponse { id, playlist_url }).into_response());
    }

    let job_id = enqueue_hls_job(&state, &id).await?;
    let body = StreamPendingResponse {
        status: "queued".to_string(),
        job_id,
    };
    Ok((StatusCode::ACCEPTED, Json(body)).into_response())
}

async fn login_handler(
    Extension(state): Extension<Arc<AppState>>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    info!(user_id = %payload.user_id, "login request");
    let user = fetch_user(&state.db, &payload.user_id).await?;
    verify_password(&payload.password, &user.password_hash)?;

    let token = issue_token(&state.auth, &user.user_id, user.role)?;

    Ok(Json(LoginResponse {
        token,
        user_id: user.user_id,
        role: user.role,
        expires_in: state.auth.token_ttl.as_secs(),
    }))
}

async fn create_user_handler(
    Extension(state): Extension<Arc<AppState>>,
    Json(payload): Json<CreateUserRequest>,
) -> Result<Json<CreateUserResponse>, ApiError> {
    if payload.invite_code != state.auth.invite_code {
        return Err(ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: "invalid invite code".to_string(),
        });
    }

    let hash = hash_password(&payload.password)?;
    let created = create_user(&state.db, &payload.user_id, &hash, UserRole::User).await?;
    info!(user_id = %created.user_id, "user created");

    Ok(Json(CreateUserResponse {
        user_id: created.user_id,
        role: created.role,
    }))
}

async fn delete_user_handler(
    Path(id): Path<String>,
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<StatusCode, ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    if auth.role != UserRole::Admin && auth.user_id != id {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "forbidden".to_string(),
        });
    }

    delete_user(&state.db, &id).await?;
    info!(user_id = %id, "user deleted");
    Ok(StatusCode::NO_CONTENT)
}

async fn auto_match_handler(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<(StatusCode, Json<JobIdResponse>), ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    if auth.role != UserRole::Admin {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "forbidden".to_string(),
        });
    }

    let job_id = enqueue_job(
        &state.db,
        "auto-match",
        json!({}),
        state.job_max_attempts,
        Some("auto-match"),
    )
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("auto match enqueue failed: {}", err),
    })?;

    Ok((StatusCode::ACCEPTED, Json(JobIdResponse { job_id })))
}

async fn enqueue_index_job_handler(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<(StatusCode, Json<JobIdResponse>), ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    if auth.role != UserRole::Admin {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "forbidden".to_string(),
        });
    }

    let job_id = enqueue_job(
        &state.db,
        "index",
        json!({"mode":"incremental"}),
        state.job_max_attempts,
        Some("index"),
    )
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("index enqueue failed: {}", err),
    })?;

    Ok((StatusCode::ACCEPTED, Json(JobIdResponse { job_id })))
}

async fn enqueue_auto_match_job_handler(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
    Json(payload): Json<AutoMatchJobRequest>,
) -> Result<(StatusCode, Json<JobIdResponse>), ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    if auth.role != UserRole::Admin {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "forbidden".to_string(),
        });
    }

    let job_id = enqueue_job(
        &state.db,
        "auto-match",
        json!({
            "limit": payload.limit,
            "min_candidate_score": payload.min_candidate_score,
            "min_confidence": payload.min_confidence,
        }),
        state.job_max_attempts,
        Some("auto-match"),
    )
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("auto match enqueue failed: {}", err),
    })?;

    Ok((StatusCode::ACCEPTED, Json(JobIdResponse { job_id })))
}

async fn enqueue_hls_job_handler(
    Path(id): Path<String>,
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<(StatusCode, Json<JobIdResponse>), ApiError> {
    require_auth(&state, &headers, &query.0, None).await?;
    let job_id = enqueue_hls_job(&state, &id).await?;
    Ok((StatusCode::ACCEPTED, Json(JobIdResponse { job_id })))
}

async fn job_status_handler(
    Path(id): Path<i64>,
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<Json<JobStatusResponse>, ApiError> {
    require_auth(&state, &headers, &query.0, None).await?;
    let job = get_job_status(&state.db, id)
        .await
        .map_err(|err| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("job lookup failed: {}", err),
        })?
        .ok_or_else(|| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "job not found".to_string(),
        })?;

    Ok(Json(JobStatusResponse { job }))
}

async fn job_status_stream_handler(
    Path(id): Path<i64>,
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    require_auth(&state, &headers, &query.0, None).await?;
    let initial = get_job_status(&state.db, id)
        .await
        .map_err(|err| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("job lookup failed: {}", err),
        })?
        .ok_or_else(|| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "job not found".to_string(),
        })?;

    let db = state.db.clone();
    let interval_ms = state.job_poll_interval_ms.max(200);
    let stream = stream! {
        let payload = serde_json::to_string(&initial).unwrap_or_default();
        yield Ok(Event::default().event(initial.status.clone()).data(payload));

        let mut ticker = tokio::time::interval(Duration::from_millis(interval_ms));
        loop {
            ticker.tick().await;
            match get_job_status(&db, id).await {
                Ok(Some(job)) => {
                    let payload = serde_json::to_string(&job).unwrap_or_default();
                    yield Ok(Event::default().event(job.status.clone()).data(payload));
                    if job.status == "done" || job.status == "failed" {
                        break;
                    }
                }
                Ok(None) => {
                    yield Ok(Event::default().event("error").data("job not found"));
                    break;
                }
                Err(err) => {
                    yield Ok(Event::default().event("error").data(err.to_string()));
                    break;
                }
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keepalive"),
    ))
}

async fn match_status_handler(
    Path(id): Path<String>,
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<Json<MatchStatusResponse>, ApiError> {
    require_auth(&state, &headers, &query.0, None).await?;
    let current = get_match(&state.db, &id).await.map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("match lookup failed: {}", err),
    })?;
    Ok(Json(MatchStatusResponse { current }))
}

async fn match_candidates_handler(
    Path(id): Path<String>,
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<Json<MatchCandidatesResponse>, ApiError> {
    require_auth(&state, &headers, &query.0, None).await?;
    let candidates = get_candidates(&state.db, &id).await.map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("candidate lookup failed: {}", err),
    })?;
    Ok(Json(MatchCandidatesResponse { candidates }))
}

async fn manual_match_handler(
    Path(id): Path<String>,
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
    Json(payload): Json<ManualMatchRequest>,
) -> Result<StatusCode, ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    if auth.role != UserRole::Admin {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "forbidden".to_string(),
        });
    }

    set_manual_match(&state.db, &id, payload.subject_id, payload.episode_id)
        .await
        .map_err(|err| ApiError {
            status: StatusCode::BAD_REQUEST,
            message: format!("manual match failed: {}", err),
        })?;

    info!(media_id = %id, subject_id = payload.subject_id, "manual match set");
    Ok(StatusCode::NO_CONTENT)
}

async fn clear_match_handler(
    Path(id): Path<String>,
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<StatusCode, ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    if auth.role != UserRole::Admin {
        return Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "forbidden".to_string(),
        });
    }

    clear_match(&state.db, &id).await.map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("clear match failed: {}", err),
    })?;
    info!(media_id = %id, "match cleared");
    Ok(StatusCode::NO_CONTENT)
}

async fn hls_file_handler(
    Path((id, file)): Path<(String, String)>,
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<Response, ApiError> {
    require_auth(&state, &headers, &query.0, None).await?;
    serve_hls_file(&state, &id, &file).await
}

async fn require_auth(
    state: &AppState,
    headers: &HeaderMap,
    query: &TokenQuery,
    token_override: Option<&str>,
) -> Result<AuthContext, ApiError> {
    let token = if let Some(token) = token_override {
        token.to_string()
    } else {
        extract_token(headers, query).ok_or_else(|| ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: "missing token".to_string(),
        })?
    };

    let claims = decode_token(&state.auth, &token)?;

    Ok(AuthContext {
        user_id: claims.sub,
        role: claims.role,
        token,
    })
}

async fn hls_file_handler_with_token(
    Path((token, id, file)): Path<(String, String, String)>,
    Extension(state): Extension<Arc<AppState>>,
) -> Result<Response, ApiError> {
    let headers = HeaderMap::new();
    let query = Query(TokenQuery { token: None });
    require_auth(&state, &headers, &query.0, Some(&token)).await?;
    serve_hls_file(&state, &id, &file).await
}

async fn serve_hls_file(state: &AppState, id: &str, file: &str) -> Result<Response, ApiError> {
    let root = state.config.hls_root();
    let file_path = root.join(id).join(file);
    let safe_path = ensure_within_root(&root, &file_path)?;

    let file = File::open(&safe_path).await.map_err(|_| ApiError {
        status: StatusCode::NOT_FOUND,
        message: "file not found".to_string(),
    })?;

    let content_type = match safe_path.extension().and_then(|ext| ext.to_str()) {
        Some("m3u8") => "application/vnd.apple.mpegurl",
        Some("ts") => "video/mp2t",
        _ => "application/octet-stream",
    };

    let stream = ReaderStream::new(file);
    let body = axum::body::Body::from_stream(stream);

    Ok(([(header::CONTENT_TYPE, content_type)], body).into_response())
}

fn apply_limits(mut app: Router, server: &anicargo_config::ServerConfig) -> Router {
    if server.max_in_flight > 0 {
        app = app.layer(ConcurrencyLimitLayer::new(
            server.max_in_flight as usize,
        ));
    }
    app
}

fn to_string_set(values: &[String]) -> HashSet<String> {
    values
        .iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn parse_ip_set(items: &[String]) -> Result<HashSet<IpAddr>, Box<dyn std::error::Error>> {
    let mut set = HashSet::new();
    for item in items {
        let value = item.trim();
        if value.is_empty() {
            continue;
        }
        let ip: IpAddr = value
            .parse()
            .map_err(|_| format!("invalid ip address: {}", value))?;
        set.insert(ip);
    }
    Ok(set)
}

fn extract_user_id(state: &AppState, headers: &HeaderMap) -> Option<String> {
    let token_query = TokenQuery { token: None };
    let token = extract_token(headers, &token_query)?;
    let claims = decode_token(&state.auth, &token).ok()?;
    Some(claims.sub)
}

async fn rate_limit_middleware(
    State(state): State<Arc<AppState>>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request<axum::body::Body>,
    next: middleware::Next,
) -> Result<Response, StatusCode> {
    let user_id = extract_user_id(&state, request.headers());
    if let Some(user_id) = &user_id {
        if state.rate_limit_block_users.contains(user_id) {
            return Ok((StatusCode::FORBIDDEN, Json(ErrorResponse {
                error: "access denied".to_string(),
            })).into_response());
        }
        if state.rate_limit_allow_users.contains(user_id) {
            return Ok(next.run(request).await);
        }
    }

    if state.rate_limit_block_ips.contains(&addr.ip()) {
        return Ok((StatusCode::FORBIDDEN, Json(ErrorResponse {
            error: "access denied".to_string(),
        })).into_response());
    }
    if state.rate_limit_allow_ips.contains(&addr.ip()) {
        return Ok(next.run(request).await);
    }

    let (key, limit) = if let Some(user_id) = &user_id {
        (format!("user:{}", user_id), state.rate_limit_user_per_minute)
    } else {
        (format!("ip:{}", addr.ip()), state.rate_limit_ip_per_minute)
    };

    if limit == 0 {
        return Ok(next.run(request).await);
    }

    let mut guard = state.rate_limits.lock().await;
    let now = Instant::now();
    let cutoff = now - Duration::from_secs(600);
    guard.retain(|_, value| value.last_seen >= cutoff);
    let entry = guard.entry(key).or_insert(RateLimitState {
        window_start: now,
        count: 0,
        last_seen: now,
    });
    if now.duration_since(entry.window_start) >= Duration::from_secs(60) {
        entry.window_start = now;
        entry.count = 0;
    }
    entry.last_seen = now;
    if entry.count >= limit {
        let body = Json(ErrorResponse {
            error: "rate limit exceeded".to_string(),
        });
        return Ok((StatusCode::TOO_MANY_REQUESTS, body).into_response());
    }
    entry.count += 1;
    drop(guard);

    Ok(next.run(request).await)
}

fn spawn_job_workers(state: Arc<AppState>, workers: u32, poll_interval: Duration) {
    for idx in 0..workers {
        let worker_state = state.clone();
        let worker_id = format!("api-{}-{}", std::process::id(), idx);
        let interval = poll_interval;
        tokio::spawn(async move {
            job_worker_loop(worker_state, worker_id, interval).await;
        });
    }
}

fn spawn_job_cleanup(state: Arc<AppState>, interval: Duration) {
    tokio::spawn(async move {
        job_cleanup_loop(state, interval).await;
    });
}

async fn job_cleanup_loop(state: Arc<AppState>, interval: Duration) {
    let mut ticker = tokio::time::interval(interval);
    loop {
        ticker.tick().await;
        if state.job_running_timeout_secs > 0 {
            match requeue_stuck_jobs(&state.db, state.job_running_timeout_secs).await {
                Ok((retried, failed)) => {
                    if retried > 0 || failed > 0 {
                        info!(retried, failed, "requeued stuck jobs");
                    }
                }
                Err(err) => {
                    tracing::warn!("requeue stuck jobs failed: {}", err);
                }
            }
        }

        if state.job_retention_hours == 0 {
            continue;
        }
        match cleanup_jobs(&state.db, state.job_retention_hours).await {
            Ok(removed) => {
                if removed > 0 {
                    info!(removed, "job cleanup removed old entries");
                }
            }
            Err(err) => {
                tracing::warn!("job cleanup failed: {}", err);
            }
        }
    }
}

async fn job_worker_loop(state: Arc<AppState>, worker_id: String, poll_interval: Duration) {
    loop {
        match fetch_next_job(&state.db, &worker_id).await {
            Ok(Some(job)) => {
                let job_id = job.id;
                let attempts = job.attempts;
                let max_attempts = job.max_attempts;
                match process_job(&state, job).await {
                    Ok(result) => {
                        if let Err(err) = complete_job(&state.db, job_id, result).await {
                            tracing::warn!(job_id, "job completion failed: {}", err);
                        }
                    }
                    Err(err) => {
                        if let Err(err) =
                            fail_job(&state.db, job_id, attempts, max_attempts, &err).await
                        {
                            tracing::warn!(job_id, "job failure update failed: {}", err);
                        }
                    }
                }
            }
            Ok(None) => sleep(poll_interval).await,
            Err(err) => {
                tracing::warn!("job polling failed: {}", err);
                sleep(poll_interval).await;
            }
        }
    }
}

async fn process_job(state: &AppState, job: Job) -> Result<Option<Value>, String> {
    match job.job_type.as_str() {
        "index" => {
            let _permit = state
                .scan_limit
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| "scan queue closed".to_string())?;
            let summary = scan_and_index(&state.db, &state.config)
                .await
                .map_err(|err| format!("index failed: {}", err))?;
            serde_json::to_value(summary)
                .map(Some)
                .map_err(|err| format!("index result encode failed: {}", err))
        }
        "auto-match" => {
            let options = auto_match_options_from_payload(&job.payload);
            let summary = auto_match_all(&state.db, &state.bangumi, options)
                .await
                .map_err(|err| format!("auto match failed: {}", err))?;
            serde_json::to_value(summary)
                .map(Some)
                .map_err(|err| format!("auto match result encode failed: {}", err))
        }
        "hls" => {
            let media_id = job
                .payload
                .get("media_id")
                .and_then(|value| value.as_str())
                .ok_or_else(|| "missing media_id".to_string())?
                .to_string();
            let _permit = state
                .hls_limit
                .clone()
                .acquire_owned()
                .await
                .map_err(|_| "hls queue closed".to_string())?;
            let _lock = acquire_hls_lock(state, &media_id).await;
            let config = state.config.clone();
            let id_clone = media_id.clone();
            tokio::task::spawn_blocking(move || {
                let entry = find_entry_by_id(&config, &id_clone)?;
                ensure_hls(&entry, &config)
            })
            .await
            .map_err(|_| "hls task failed".to_string())?
            .map_err(|err| err.to_string())?;

            Ok(Some(json!({ "media_id": media_id })))
        }
        _ => Err(format!("unknown job type: {}", job.job_type)),
    }
}

fn auto_match_options_from_payload(payload: &Value) -> AutoMatchOptions {
    let mut options = AutoMatchOptions::default();
    if let Some(value) = payload.get("limit").and_then(|value| value.as_u64()) {
        options.limit = value as u32;
    }
    if let Some(value) = payload
        .get("min_candidate_score")
        .and_then(|value| value.as_f64())
    {
        options.min_candidate_score = value as f32;
    }
    if let Some(value) = payload
        .get("min_confidence")
        .and_then(|value| value.as_f64())
    {
        options.min_confidence = value as f32;
    }
    options
}

async fn enqueue_hls_job(state: &AppState, media_id: &str) -> Result<i64, ApiError> {
    if media_id.trim().is_empty() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "missing media id".to_string(),
        });
    }
    let job_id = enqueue_job(
        &state.db,
        "hls",
        json!({ "media_id": media_id }),
        state.job_max_attempts,
        Some(media_id),
    )
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("hls enqueue failed: {}", err),
    })?;
    Ok(job_id)
}

async fn acquire_hls_lock(state: &AppState, id: &str) -> tokio::sync::OwnedMutexGuard<()> {
    let lock = {
        let mut locks = state.hls_locks.lock().await;
        locks
            .entry(id.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };
    lock.lock_owned().await
}

struct AuthContext {
    user_id: String,
    role: UserRole,
    token: String,
}

fn extract_token(headers: &HeaderMap, query: &TokenQuery) -> Option<String> {
    if let Some(token) = &query.token {
        return Some(token.clone());
    }

    let header_value = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    let token = header_value.strip_prefix("Bearer ")?;
    Some(token.to_string())
}

async fn connect_db(url: &str, max_connections: u32) -> Result<PgPool, ApiError> {
    if url.trim().is_empty() {
        return Err(ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "missing database url".to_string(),
        });
    }

    PgPoolOptions::new()
        .max_connections(max_connections)
        .connect(url)
        .await
        .map_err(|err| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("database connection failed: {}", err),
        })
}

async fn init_db(db: &PgPool) -> Result<(), ApiError> {
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS users (\
            id TEXT PRIMARY KEY,\
            password_hash TEXT NOT NULL,\
            role TEXT NOT NULL,\
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()\
        )",
    )
    .execute(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to init db: {}", err),
    })?;

    init_library(db).await.map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to init library: {}", err),
    })?;

    Ok(())
}

async fn ensure_admin(db: &PgPool, auth: &AuthConfig) -> Result<(), ApiError> {
    let hash = hash_password(&auth.admin_password)?;

    sqlx::query(
        "INSERT INTO users (id, password_hash, role) VALUES ($1, $2, $3) \
        ON CONFLICT (id) DO NOTHING",
    )
    .bind(&auth.admin_user)
    .bind(&hash)
    .bind("admin")
    .execute(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to ensure admin user: {}", err),
    })?;

    Ok(())
}

struct DbUser {
    user_id: String,
    password_hash: String,
    role: UserRole,
}

async fn fetch_user(db: &PgPool, user_id: &str) -> Result<DbUser, ApiError> {
    let row = sqlx::query_as::<_, (String, String, String)>(
        "SELECT id, password_hash, role FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to fetch user: {}", err),
    })?
    .ok_or_else(|| ApiError {
        status: StatusCode::UNAUTHORIZED,
        message: "invalid credentials".to_string(),
    })?;

    let role = parse_role(&row.2)?;

    Ok(DbUser {
        user_id: row.0,
        password_hash: row.1,
        role,
    })
}

async fn create_user(
    db: &PgPool,
    user_id: &str,
    password_hash: &str,
    role: UserRole,
) -> Result<DbUser, ApiError> {
    let role_str = role_to_str(role);
    let row = sqlx::query_as::<_, (String, String, String)>(
        "INSERT INTO users (id, password_hash, role) VALUES ($1, $2, $3) \
        RETURNING id, password_hash, role",
    )
    .bind(user_id)
    .bind(password_hash)
    .bind(role_str)
    .fetch_one(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::BAD_REQUEST,
        message: format!("failed to create user: {}", err),
    })?;

    let role = parse_role(&row.2)?;

    Ok(DbUser {
        user_id: row.0,
        password_hash: row.1,
        role,
    })
}

async fn delete_user(db: &PgPool, user_id: &str) -> Result<(), ApiError> {
    let result = sqlx::query("DELETE FROM users WHERE id = $1")
        .bind(user_id)
        .execute(db)
        .await
        .map_err(|err| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("failed to delete user: {}", err),
        })?;

    if result.rows_affected() == 0 {
        return Err(ApiError {
            status: StatusCode::NOT_FOUND,
            message: "user not found".to_string(),
        });
    }

    Ok(())
}

fn parse_role(role: &str) -> Result<UserRole, ApiError> {
    match role {
        "admin" => Ok(UserRole::Admin),
        "user" => Ok(UserRole::User),
        _ => Err(ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "invalid role".to_string(),
        }),
    }
}

fn role_to_str(role: UserRole) -> &'static str {
    match role {
        UserRole::Admin => "admin",
        UserRole::User => "user",
    }
}

fn hash_password(password: &str) -> Result<String, ApiError> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    argon2
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "failed to hash password".to_string(),
        })
}

fn verify_password(password: &str, hash: &str) -> Result<(), ApiError> {
    let parsed_hash = PasswordHash::new(hash).map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "invalid password hash".to_string(),
    })?;

    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .map_err(|_| ApiError {
            status: StatusCode::UNAUTHORIZED,
            message: "invalid credentials".to_string(),
        })
}

fn issue_token(auth: &AuthConfig, user_id: &str, role: UserRole) -> Result<String, ApiError> {
    let exp = SystemTime::now()
        .checked_add(auth.token_ttl)
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .ok_or_else(|| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "failed to build token".to_string(),
        })?;

    let claims = Claims {
        sub: user_id.to_string(),
        role,
        exp,
    };

    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(auth.jwt_secret.as_bytes()),
    )
    .map_err(|_| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: "failed to issue token".to_string(),
    })
}

fn decode_token(auth: &AuthConfig, token: &str) -> Result<Claims, ApiError> {
    jsonwebtoken::decode::<Claims>(
        token,
        &DecodingKey::from_secret(auth.jwt_secret.as_bytes()),
        &Validation::default(),
    )
    .map(|data| data.claims)
    .map_err(|_| ApiError {
        status: StatusCode::UNAUTHORIZED,
        message: "invalid token".to_string(),
    })
}

fn ensure_within_root(root: &StdPath, path: &StdPath) -> Result<PathBuf, ApiError> {
    let root = root
        .canonicalize()
        .map_err(|_| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "invalid hls root".to_string(),
        })?;
    let candidate = path
        .canonicalize()
        .map_err(|_| ApiError {
            status: StatusCode::NOT_FOUND,
            message: "file not found".to_string(),
        })?;

    if candidate.starts_with(&root) {
        Ok(candidate)
    } else {
        Err(ApiError {
            status: StatusCode::FORBIDDEN,
            message: "invalid path".to_string(),
        })
    }
}
