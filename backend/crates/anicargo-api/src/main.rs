use anicargo_bangumi::BangumiClient;
use anicargo_config::{init_logging, split_config_args, AppConfig};
use anicargo_library::{
    auto_match_all, auto_match_unmatched, cleanup_jobs, clear_match, complete_job, enqueue_job,
    fail_job, fetch_next_job, get_candidates, get_job_status, get_match, init_library,
    list_media_entries, requeue_stuck_jobs, scan_and_index, set_manual_match, sync_bangumi_subject,
    AutoMatchOptions, Job, JobStatus, MatchCandidate, MediaMatch,
};
use anicargo_media::{ensure_hls, find_entry_by_id, MediaConfig, MediaError, MediaEntry};
use anicargo_qbittorrent::{QbittorrentClient, QbittorrentError};
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::Argon2;
use axum::extract::{ConnectInfo, Multipart, Path, Query, State};
use axum::http::{header, HeaderMap, Request, StatusCode};
use axum::middleware;
use axum::response::{IntoResponse, Response};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{delete, get, patch, post};
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
use sqlx::{FromRow, PgPool};
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::env;
use std::fmt;
use std::fs;
use std::cmp::Ordering as CmpOrdering;
use std::net::{IpAddr, SocketAddr};
use std::path::{Path as StdPath, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::fs::File;
use tokio::sync::{Mutex, Semaphore};
use tokio::time::sleep;
use tokio_util::io::ReaderStream;
use tower::limit::ConcurrencyLimitLayer;
use tracing::info;
use sha2::{Digest, Sha256};
use sysinfo::{Disks, Networks, System};

#[derive(Clone)]
struct AppState {
    config: Arc<MediaConfig>,
    db: PgPool,
    auth: Arc<AuthConfig>,
    bangumi: Arc<BangumiClient>,
    qbittorrent: Option<QbittorrentClient>,
    qbittorrent_download_dir: Option<PathBuf>,
    started_at: SystemTime,
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
    in_flight: Arc<AtomicUsize>,
    metrics_state: Arc<Mutex<MetricsState>>,
    max_in_flight: u32,
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

#[derive(Debug)]
struct MetricsState {
    last_network: Option<NetworkSnapshot>,
}

impl Default for MetricsState {
    fn default() -> Self {
        Self { last_network: None }
    }
}

#[derive(Debug, Clone)]
struct NetworkSnapshot {
    rx_bytes: u64,
    tx_bytes: u64,
    at: Instant,
}

struct InFlightGuard {
    counter: Arc<AtomicUsize>,
}

impl InFlightGuard {
    fn new(counter: Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::Relaxed);
        Self { counter }
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.counter.fetch_sub(1, Ordering::Relaxed);
    }
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
    role_level: i32,
    expires_in: u64,
}

#[derive(Debug, Serialize)]
struct CreateUserResponse {
    user_id: String,
    role: UserRole,
    role_level: i32,
}

#[derive(Debug, Serialize)]
struct UpdateRoleResponse {
    user_id: String,
    role: UserRole,
    role_level: i32,
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
struct UpdateRoleRequest {
    role_level: i32,
}

#[derive(Debug, Deserialize)]
struct CollectionMagnetRequest {
    magnet: String,
    note: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CollectionQuery {
    token: Option<String>,
    status: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AdminJobsQuery {
    token: Option<String>,
    status: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct UpdateSettingsRequest {
    display_name: Option<String>,
    theme: Option<String>,
    playback_speed: Option<f64>,
    subtitle_lang: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateProgressRequest {
    position_secs: f64,
    duration_secs: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct ProgressQuery {
    token: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct CollectionDecisionRequest {
    note: Option<String>,
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
struct QbittorrentMagnetRequest {
    magnet: String,
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

#[derive(Debug, Serialize)]
struct QbittorrentAddResponse {
    status: String,
}

#[derive(Debug, Serialize)]
struct QbittorrentTorrentInfo {
    hash: String,
    name: String,
    state: String,
    progress: f32,
    save_path: Option<String>,
    content_path: Option<String>,
    completion_on: Option<i64>,
}

#[derive(Debug, Serialize)]
struct UserSettingsResponse {
    display_name: Option<String>,
    theme: String,
    playback_speed: f64,
    subtitle_lang: Option<String>,
}

#[derive(Debug, Serialize)]
struct MediaProgressResponse {
    media_id: String,
    position_secs: f64,
    duration_secs: Option<f64>,
}

#[derive(Debug, Serialize)]
struct MediaProgressItem {
    media_id: String,
    filename: String,
    position_secs: f64,
    duration_secs: Option<f64>,
    updated_at: String,
}

#[derive(Debug, Serialize)]
struct MediaProgressListResponse {
    items: Vec<MediaProgressItem>,
}

#[derive(Debug, Serialize)]
struct MediaDetailResponse {
    entry: MediaEntry,
    parse: Option<MediaParseInfo>,
    matched: Option<MediaMatchDetail>,
    progress: Option<MediaProgressResponse>,
}

#[derive(Debug, Serialize)]
struct MediaParseInfo {
    title: Option<String>,
    episode: Option<String>,
    season: Option<String>,
    year: Option<String>,
    release_group: Option<String>,
    resolution: Option<String>,
}

#[derive(Debug, Serialize)]
struct MediaMatchDetail {
    subject: BangumiSubjectInfo,
    episode: Option<BangumiEpisodeInfo>,
    method: String,
    confidence: Option<f32>,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct BangumiSubjectInfo {
    id: i64,
    name: String,
    name_cn: String,
    air_date: Option<String>,
    total_episodes: Option<i64>,
}

#[derive(Debug, Serialize, Clone)]
struct BangumiEpisodeInfo {
    id: i64,
    sort: f64,
    ep: Option<f64>,
    name: String,
    name_cn: String,
    air_date: Option<String>,
}

#[derive(Debug, Serialize)]
struct EpisodeListResponse {
    subject: BangumiSubjectInfo,
    episodes: Vec<BangumiEpisodeInfo>,
}

#[derive(Debug, Serialize)]
struct NextMediaEntry {
    id: String,
    filename: String,
    size: u64,
}

#[derive(Debug, Serialize)]
struct NextEpisodeResponse {
    subject: Option<BangumiSubjectInfo>,
    current_episode: Option<BangumiEpisodeInfo>,
    next_episode: Option<BangumiEpisodeInfo>,
    next_media: Option<NextMediaEntry>,
}

#[derive(Debug, Serialize)]
struct AdminMetricsResponse {
    uptime_secs: u64,
    media_count: i64,
    media_total_bytes: i64,
    job_counts: JobCounts,
    system: SystemMetrics,
    storage: StorageMetrics,
    network: NetworkMetrics,
    in_flight_requests: usize,
    max_in_flight: u32,
    qbittorrent: Option<QbittorrentTransferMetrics>,
}

#[derive(Debug, Serialize)]
struct JobCounts {
    queued: i64,
    running: i64,
    retry: i64,
    done: i64,
    failed: i64,
}

#[derive(Debug, Serialize)]
struct SystemMetrics {
    total_memory_bytes: u64,
    used_memory_bytes: u64,
    process_memory_bytes: u64,
    cpu_usage_percent: f32,
}

#[derive(Debug, Serialize)]
struct StorageMetrics {
    media_dir: Option<DiskUsage>,
    cache_dir: Option<DiskUsage>,
    qbittorrent_download_dir: Option<DiskUsage>,
}

#[derive(Debug, Serialize)]
struct DiskUsage {
    mount_point: String,
    total_bytes: u64,
    available_bytes: u64,
}

#[derive(Debug, Serialize)]
struct NetworkMetrics {
    rx_bytes: u64,
    tx_bytes: u64,
    rx_bytes_per_sec: f64,
    tx_bytes_per_sec: f64,
    interfaces: Vec<NetworkInterfaceMetrics>,
}

#[derive(Debug, Serialize)]
struct NetworkInterfaceMetrics {
    name: String,
    rx_bytes: u64,
    tx_bytes: u64,
}

#[derive(Debug, Serialize)]
struct QbittorrentTransferMetrics {
    download_speed_bytes: u64,
    upload_speed_bytes: u64,
    download_total_bytes: u64,
    upload_total_bytes: u64,
    download_rate_limit: i64,
    upload_rate_limit: i64,
    dht_nodes: i64,
    connection_status: String,
}

#[derive(Debug, Serialize)]
struct JobQueueItem {
    id: i64,
    job_type: String,
    status: String,
    attempts: i32,
    max_attempts: i32,
    payload: Value,
    result: Option<Value>,
    last_error: Option<String>,
    scheduled_at: String,
    locked_at: Option<String>,
    locked_by: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Serialize)]
struct JobQueueResponse {
    jobs: Vec<JobQueueItem>,
}

#[derive(Debug, Serialize)]
struct UserSummary {
    user_id: String,
    role: UserRole,
    role_level: i32,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct CollectionItem {
    id: i64,
    submitter_id: String,
    kind: String,
    status: String,
    magnet: Option<String>,
    torrent_name: Option<String>,
    note: Option<String>,
    decision_note: Option<String>,
    created_at: String,
    decided_at: Option<String>,
    decided_by: Option<String>,
}

#[derive(Debug, Serialize)]
struct CollectionListResponse {
    items: Vec<CollectionItem>,
}

#[derive(Debug, Serialize)]
struct CollectionCreateResponse {
    id: i64,
    status: String,
}

#[derive(Debug, Serialize)]
struct CollectionApproveResponse {
    id: i64,
    status: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum UserRole {
    Admin,
    User,
}

const ROLE_LEVEL_MIN: i32 = 1;
const ROLE_LEVEL_MAX: i32 = 5;
const ROLE_LEVEL_ADMIN: i32 = 3;
const ROLE_LEVEL_SUPER_ADMIN: i32 = 5;
const MAX_TORRENT_BYTES: usize = 4 * 1024 * 1024;

fn role_from_level(level: i32) -> UserRole {
    if level >= ROLE_LEVEL_ADMIN {
        UserRole::Admin
    } else {
        UserRole::User
    }
}

fn normalize_role_level(level: i32) -> i32 {
    level.clamp(ROLE_LEVEL_MIN, ROLE_LEVEL_MAX)
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    role_level: i32,
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

fn not_found_error() -> ApiError {
    ApiError {
        status: StatusCode::NOT_FOUND,
        message: "not found".to_string(),
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
    let qbittorrent = if app_config.qbittorrent.base_url.trim().is_empty() {
        None
    } else {
        Some(QbittorrentClient::new(
            app_config.qbittorrent.base_url.clone(),
            app_config.qbittorrent.username.clone(),
            app_config.qbittorrent.password.clone(),
        )?)
    };
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
        qbittorrent,
        qbittorrent_download_dir: app_config.qbittorrent.download_dir.clone(),
        started_at: SystemTime::now(),
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
        in_flight: Arc::new(AtomicUsize::new(0)),
        metrics_state: Arc::new(Mutex::new(MetricsState::default())),
        max_in_flight: app_config.server.max_in_flight,
        job_poll_interval_ms: app_config.server.job_poll_interval_ms,
        job_retention_hours: app_config.server.job_retention_hours,
        job_running_timeout_secs: app_config.server.job_running_timeout_secs,
        job_max_attempts: app_config.server.job_max_attempts,
    });

    let mut app = Router::new()
        .route("/api/library", get(library_handler))
        .route("/api/stream/:id", get(stream_handler))
        .route("/api/auth/login", post(login_handler))
        .route("/api/users", post(create_user_handler).get(list_users_handler))
        .route("/api/users/:id", delete(delete_user_handler))
        .route("/api/users/:id/role", patch(update_user_role_handler))
        .route("/api/settings", get(get_settings_handler).put(update_settings_handler))
        .route("/api/progress", get(list_progress_handler))
        .route("/api/progress/:id", get(get_progress_handler).put(update_progress_handler))
        .route("/api/media/:id", get(media_detail_handler))
        .route("/api/media/:id/next", get(next_episode_handler))
        .route("/api/media/:id/episodes", get(media_episodes_handler))
        .route("/api/subjects/:id/episodes", get(subject_episodes_handler))
        .route("/api/collection", get(list_collection_handler))
        .route("/api/collection/magnet", post(collection_magnet_handler))
        .route("/api/collection/torrent", post(collection_torrent_handler))
        .route("/api/collection/:id/approve", post(collection_approve_handler))
        .route("/api/collection/:id/reject", post(collection_reject_handler))
        .route("/api/collection/:id", delete(delete_collection_handler))
        .route("/api/admin/metrics", get(admin_metrics_handler))
        .route("/api/admin/jobs", get(admin_jobs_handler))
        .route("/api/admin/qbittorrent/completed", get(admin_qbittorrent_completed_handler))
        .route("/api/matches/auto", post(auto_match_handler))
        .route("/api/matches/:id", get(match_status_handler).post(manual_match_handler).delete(clear_match_handler))
        .route(
            "/api/matches/:id/candidates",
            get(match_candidates_handler),
        )
        .route("/api/qbittorrent/magnet", post(qbittorrent_magnet_handler))
        .route("/api/qbittorrent/torrent", post(qbittorrent_torrent_handler))
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
    app = app.layer(middleware::from_fn_with_state(
        state.clone(),
        in_flight_middleware,
    ));

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
        if auth.role_level < ROLE_LEVEL_ADMIN {
            return Err(not_found_error());
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

    let token = issue_token(&state.auth, &user.user_id, user.role_level)?;

    Ok(Json(LoginResponse {
        token,
        user_id: user.user_id,
        role: user.role,
        role_level: user.role_level,
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
    let created = create_user(&state.db, &payload.user_id, &hash, ROLE_LEVEL_MIN).await?;
    info!(user_id = %created.user_id, "user created");

    Ok(Json(CreateUserResponse {
        user_id: created.user_id,
        role: created.role,
        role_level: created.role_level,
    }))
}

async fn list_users_handler(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<Json<Vec<UserSummary>>, ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    if auth.role_level < ROLE_LEVEL_ADMIN {
        return Err(not_found_error());
    }

    let users = list_users(&state.db).await?;
    Ok(Json(users))
}

async fn delete_user_handler(
    Path(id): Path<String>,
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<StatusCode, ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    if auth.role_level < ROLE_LEVEL_ADMIN && auth.user_id != id {
        return Err(not_found_error());
    }

    delete_user(&state.db, &id).await?;
    info!(user_id = %id, "user deleted");
    Ok(StatusCode::NO_CONTENT)
}

async fn update_user_role_handler(
    Path(id): Path<String>,
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
    Json(payload): Json<UpdateRoleRequest>,
) -> Result<Json<UpdateRoleResponse>, ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    if auth.role_level < ROLE_LEVEL_ADMIN {
        return Err(not_found_error());
    }
    if auth.user_id == id {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "cannot modify own role".to_string(),
        });
    }

    let next_level = payload.role_level;
    if next_level < ROLE_LEVEL_MIN || next_level > ROLE_LEVEL_MAX {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "invalid role level".to_string(),
        });
    }
    if next_level >= auth.role_level {
        return Err(not_found_error());
    }

    let updated = update_user_role(&state.db, &id, next_level).await?;
    info!(user_id = %id, role_level = updated.role_level, "user role updated");
    Ok(Json(UpdateRoleResponse {
        user_id: updated.user_id,
        role: updated.role,
        role_level: updated.role_level,
    }))
}

async fn get_settings_handler(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<Json<UserSettingsResponse>, ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    let settings = fetch_user_settings(&state.db, &auth.user_id).await?;
    Ok(Json(settings))
}

async fn update_settings_handler(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
    Json(payload): Json<UpdateSettingsRequest>,
) -> Result<Json<UserSettingsResponse>, ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    let settings = upsert_user_settings(&state.db, &auth.user_id, payload).await?;
    Ok(Json(settings))
}

async fn get_progress_handler(
    Path(id): Path<String>,
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<Json<MediaProgressResponse>, ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    let progress = fetch_media_progress(&state.db, &auth.user_id, &id).await?;
    Ok(Json(progress))
}

async fn update_progress_handler(
    Path(id): Path<String>,
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
    Json(payload): Json<UpdateProgressRequest>,
) -> Result<Json<MediaProgressResponse>, ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    let progress = upsert_media_progress(&state.db, &auth.user_id, &id, payload).await?;
    Ok(Json(progress))
}

async fn list_progress_handler(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<ProgressQuery>,
) -> Result<Json<MediaProgressListResponse>, ApiError> {
    let token_query = TokenQuery {
        token: query.token.clone(),
    };
    let auth = require_auth(&state, &headers, &token_query, None).await?;
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let offset = query.offset.unwrap_or(0).max(0);
    let items = list_media_progress(&state.db, &auth.user_id, limit, offset).await?;
    Ok(Json(MediaProgressListResponse { items }))
}

async fn media_detail_handler(
    Path(id): Path<String>,
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<Json<MediaDetailResponse>, ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    let entry = fetch_media_entry(&state.db, &id).await?;
    let parse = fetch_media_parse(&state.db, &id).await?;
    let match_row = fetch_media_match(&state.db, &id).await?;
    let matched = if let Some(row) = match_row {
        let subject = fetch_bangumi_subject(&state.db, row.subject_id).await?;
        let episode = if let Some(ep_id) = row.episode_id {
            fetch_bangumi_episode(&state.db, ep_id).await?
        } else {
            None
        };
        Some(MediaMatchDetail {
            subject,
            episode,
            method: row.method,
            confidence: row.confidence,
            reason: row.reason,
        })
    } else {
        None
    };
    let progress = Some(fetch_media_progress(&state.db, &auth.user_id, &id).await?);

    Ok(Json(MediaDetailResponse {
        entry,
        parse,
        matched,
        progress,
    }))
}

async fn media_episodes_handler(
    Path(id): Path<String>,
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<Json<EpisodeListResponse>, ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    let match_row = fetch_media_match(&state.db, &id).await?;
    let match_row = match_row.ok_or_else(|| ApiError {
        status: StatusCode::NOT_FOUND,
        message: "media not matched".to_string(),
    })?;
    let subject_id = match_row.subject_id;
    maybe_sync_episodes(&state, subject_id).await?;
    let subject = fetch_bangumi_subject(&state.db, subject_id).await?;
    let episodes = list_bangumi_episodes(&state.db, subject_id).await?;
    let _ = auth;
    Ok(Json(EpisodeListResponse { subject, episodes }))
}

async fn next_episode_handler(
    Path(id): Path<String>,
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<Json<NextEpisodeResponse>, ApiError> {
    require_auth(&state, &headers, &query.0, None).await?;
    let match_row = fetch_media_match(&state.db, &id).await?;
    let Some(match_row) = match_row else {
        return Ok(Json(NextEpisodeResponse {
            subject: None,
            current_episode: None,
            next_episode: None,
            next_media: None,
        }));
    };

    let subject_id = match_row.subject_id;
    maybe_sync_episodes(&state, subject_id).await?;
    let subject = fetch_bangumi_subject(&state.db, subject_id).await?;
    let episodes = list_bangumi_episodes(&state.db, subject_id).await?;
    if episodes.is_empty() {
        return Ok(Json(NextEpisodeResponse {
            subject: Some(subject),
            current_episode: None,
            next_episode: None,
            next_media: None,
        }));
    }

    let parse = fetch_media_parse(&state.db, &id).await?;
    let parsed_episode = parse
        .as_ref()
        .and_then(|row| parse_episode_number(row.episode.as_deref()));

    let current_episode = match_row
        .episode_id
        .and_then(|ep_id| episodes.iter().find(|ep| ep.id == ep_id).cloned())
        .or_else(|| {
            parsed_episode.and_then(|value| find_episode_by_number(&episodes, value))
        });

    let current_sort = if let Some(ep) = &current_episode {
        Some(ep.sort)
    } else {
        parsed_episode
    };

    let next_episode = current_sort.and_then(|current| find_next_episode(&episodes, current));
    let next_media = if let Some(next) = &next_episode {
        fetch_media_for_episode(&state.db, subject_id, next.id).await?
    } else {
        None
    };

    Ok(Json(NextEpisodeResponse {
        subject: Some(subject),
        current_episode,
        next_episode,
        next_media,
    }))
}

async fn subject_episodes_handler(
    Path(id): Path<i64>,
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<Json<EpisodeListResponse>, ApiError> {
    let _auth = require_auth(&state, &headers, &query.0, None).await?;
    maybe_sync_episodes(&state, id).await?;
    let subject = fetch_bangumi_subject(&state.db, id).await?;
    let episodes = list_bangumi_episodes(&state.db, id).await?;
    Ok(Json(EpisodeListResponse { subject, episodes }))
}

async fn list_collection_handler(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<CollectionQuery>,
) -> Result<Json<CollectionListResponse>, ApiError> {
    let token_query = TokenQuery {
        token: query.token.clone(),
    };
    let auth = require_auth(&state, &headers, &token_query, None).await?;
    if auth.role_level < 2 {
        return Err(not_found_error());
    }

    let items = list_collection_items(&state.db, &auth, query.status.as_deref()).await?;
    Ok(Json(CollectionListResponse { items }))
}

async fn collection_magnet_handler(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
    Json(payload): Json<CollectionMagnetRequest>,
) -> Result<(StatusCode, Json<CollectionCreateResponse>), ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    if auth.role_level < 2 {
        return Err(not_found_error());
    }

    let id = create_collection_magnet(&state.db, &auth.user_id, &payload.magnet, payload.note)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(CollectionCreateResponse {
            id,
            status: "pending".to_string(),
        }),
    ))
}

async fn collection_torrent_handler(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<CollectionCreateResponse>), ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    if auth.role_level < 2 {
        return Err(not_found_error());
    }

    let mut note: Option<String> = None;
    let mut file_name: Option<String> = None;
    let mut file_bytes: Option<Vec<u8>> = None;

    while let Some(field) = multipart.next_field().await.map_err(|err| ApiError {
        status: StatusCode::BAD_REQUEST,
        message: format!("invalid multipart data: {}", err),
    })? {
        let name = field.name().unwrap_or("");
        if name == "note" {
            note = field.text().await.ok();
            continue;
        }
        if name != "torrent" && name != "file" {
            continue;
        }

        let filename = field.file_name().unwrap_or("upload.torrent").to_string();
        let bytes = field.bytes().await.map_err(|err| ApiError {
            status: StatusCode::BAD_REQUEST,
            message: format!("failed to read torrent file: {}", err),
        })?;
        file_name = Some(filename);
        file_bytes = Some(bytes.to_vec());
        break;
    }

    let filename = file_name.ok_or_else(|| ApiError {
        status: StatusCode::BAD_REQUEST,
        message: "missing torrent file".to_string(),
    })?;
    if !filename.to_lowercase().ends_with(".torrent") {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "invalid torrent filename".to_string(),
        });
    }

    let bytes = file_bytes.unwrap_or_default();
    if bytes.len() > MAX_TORRENT_BYTES {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "torrent file too large".to_string(),
        });
    }
    let id = create_collection_torrent(&state.db, &auth.user_id, &filename, bytes, note).await?;
    Ok((
        StatusCode::CREATED,
        Json(CollectionCreateResponse {
            id,
            status: "pending".to_string(),
        }),
    ))
}

async fn collection_approve_handler(
    Path(id): Path<i64>,
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
    payload: Option<Json<CollectionDecisionRequest>>,
) -> Result<Json<CollectionApproveResponse>, ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    if auth.role_level < ROLE_LEVEL_ADMIN {
        return Err(not_found_error());
    }

    let client = state.qbittorrent.as_ref().ok_or_else(|| ApiError {
        status: StatusCode::SERVICE_UNAVAILABLE,
        message: "qbittorrent not configured".to_string(),
    })?;
    let save_path = state
        .qbittorrent_download_dir
        .as_ref()
        .and_then(|path| path.to_str());

    let submission = fetch_collection_item(&state.db, id).await?;
    if submission.status != "pending" {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            message: "submission already processed".to_string(),
        });
    }

    match submission.kind.as_str() {
        "magnet" => {
            let magnet = submission.magnet.as_deref().unwrap_or("");
            client
                .add_magnet(magnet, save_path)
                .await
                .map_err(map_qbittorrent_error)?;
        }
        "torrent" => {
            let filename = submission.torrent_name.as_deref().unwrap_or("upload.torrent");
            let bytes = submission.torrent_bytes.unwrap_or_default();
            client
                .add_torrent_bytes(filename, bytes, save_path)
                .await
                .map_err(map_qbittorrent_error)?;
        }
        _ => {
            return Err(ApiError {
                status: StatusCode::BAD_REQUEST,
                message: "invalid submission type".to_string(),
            });
        }
    }

    let note = payload.and_then(|value| value.0.note);
    let updated = approve_collection_item(&state.db, id, &auth.user_id, note).await?;
    Ok(Json(CollectionApproveResponse {
        id: updated.id,
        status: updated.status,
    }))
}

async fn collection_reject_handler(
    Path(id): Path<i64>,
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
    payload: Option<Json<CollectionDecisionRequest>>,
) -> Result<Json<CollectionApproveResponse>, ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    if auth.role_level < ROLE_LEVEL_ADMIN {
        return Err(not_found_error());
    }

    let note = payload.and_then(|value| value.0.note);
    let updated = reject_collection_item(&state.db, id, &auth.user_id, note).await?;
    Ok(Json(CollectionApproveResponse {
        id: updated.id,
        status: updated.status,
    }))
}

async fn delete_collection_handler(
    Path(id): Path<i64>,
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<StatusCode, ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    delete_collection_item(&state.db, &auth, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn admin_metrics_handler(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<Json<AdminMetricsResponse>, ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    if auth.role_level < ROLE_LEVEL_ADMIN {
        return Err(not_found_error());
    }

    let (media_count, media_total_bytes) = fetch_media_stats(&state.db).await?;
    let job_counts = fetch_job_counts(&state.db).await?;
    let uptime_secs = SystemTime::now()
        .duration_since(state.started_at)
        .unwrap_or_default()
        .as_secs();
    let system = collect_system_metrics();
    let storage = collect_storage_metrics(&state);
    let network = collect_network_metrics(&state).await;
    let in_flight_requests = state.in_flight.load(Ordering::Relaxed);
    let max_in_flight = state.max_in_flight;
    let qbittorrent = collect_qbittorrent_metrics(&state).await;

    Ok(Json(AdminMetricsResponse {
        uptime_secs,
        media_count,
        media_total_bytes,
        job_counts,
        system,
        storage,
        network,
        in_flight_requests,
        max_in_flight,
        qbittorrent,
    }))
}

async fn admin_jobs_handler(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<AdminJobsQuery>,
) -> Result<Json<JobQueueResponse>, ApiError> {
    let token_query = TokenQuery {
        token: query.token.clone(),
    };
    let auth = require_auth(&state, &headers, &token_query, None).await?;
    if auth.role_level < ROLE_LEVEL_ADMIN {
        return Err(not_found_error());
    }

    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let offset = query.offset.unwrap_or(0).max(0);
    let jobs = list_job_queue(&state.db, query.status.as_deref(), limit, offset).await?;
    Ok(Json(JobQueueResponse { jobs }))
}

async fn admin_qbittorrent_completed_handler(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<Json<Vec<QbittorrentTorrentInfo>>, ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    if auth.role_level < ROLE_LEVEL_ADMIN {
        return Err(not_found_error());
    }
    let client = state.qbittorrent.as_ref().ok_or_else(|| ApiError {
        status: StatusCode::SERVICE_UNAVAILABLE,
        message: "qbittorrent not configured".to_string(),
    })?;
    let items = client.list_completed().await.map_err(map_qbittorrent_error)?;
    let mapped = items
        .into_iter()
        .map(|item| QbittorrentTorrentInfo {
            hash: item.hash,
            name: item.name,
            state: item.state,
            progress: item.progress,
            save_path: item.save_path,
            content_path: item.content_path,
            completion_on: item.completion_on,
        })
        .collect();
    Ok(Json(mapped))
}

async fn auto_match_handler(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
) -> Result<(StatusCode, Json<JobIdResponse>), ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    if auth.role_level < ROLE_LEVEL_ADMIN {
        return Err(not_found_error());
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
    if auth.role_level < ROLE_LEVEL_ADMIN {
        return Err(not_found_error());
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
    if auth.role_level < ROLE_LEVEL_ADMIN {
        return Err(not_found_error());
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
    if auth.role_level < ROLE_LEVEL_ADMIN {
        return Err(not_found_error());
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
    if auth.role_level < ROLE_LEVEL_ADMIN {
        return Err(not_found_error());
    }

    clear_match(&state.db, &id).await.map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("clear match failed: {}", err),
    })?;
    info!(media_id = %id, "match cleared");
    Ok(StatusCode::NO_CONTENT)
}

async fn qbittorrent_magnet_handler(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
    Json(payload): Json<QbittorrentMagnetRequest>,
) -> Result<(StatusCode, Json<QbittorrentAddResponse>), ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    if auth.role_level < ROLE_LEVEL_ADMIN {
        return Err(not_found_error());
    }

    let client = state.qbittorrent.as_ref().ok_or_else(|| ApiError {
        status: StatusCode::SERVICE_UNAVAILABLE,
        message: "qbittorrent not configured".to_string(),
    })?;
    let magnet = payload.magnet.trim();
    if !magnet.starts_with("magnet:") {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "invalid magnet link".to_string(),
        });
    }
    let save_path = state
        .qbittorrent_download_dir
        .as_ref()
        .and_then(|path| path.to_str());
    client
        .add_magnet(magnet, save_path)
        .await
        .map_err(map_qbittorrent_error)?;

    Ok((
        StatusCode::ACCEPTED,
        Json(QbittorrentAddResponse {
            status: "queued".to_string(),
        }),
    ))
}

async fn qbittorrent_torrent_handler(
    Extension(state): Extension<Arc<AppState>>,
    headers: HeaderMap,
    query: Query<TokenQuery>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<QbittorrentAddResponse>), ApiError> {
    let auth = require_auth(&state, &headers, &query.0, None).await?;
    if auth.role_level < ROLE_LEVEL_ADMIN {
        return Err(not_found_error());
    }

    let client = state.qbittorrent.as_ref().ok_or_else(|| ApiError {
        status: StatusCode::SERVICE_UNAVAILABLE,
        message: "qbittorrent not configured".to_string(),
    })?;

    let mut file_name = None;
    let mut file_bytes: Option<Vec<u8>> = None;
    while let Some(field) = multipart.next_field().await.map_err(|err| ApiError {
        status: StatusCode::BAD_REQUEST,
        message: format!("invalid multipart data: {}", err),
    })? {
        let name = field.name().unwrap_or("");
        if name != "torrent" && name != "file" {
            continue;
        }

        let filename = field.file_name().unwrap_or("upload.torrent").to_string();
        let bytes = field.bytes().await.map_err(|err| ApiError {
            status: StatusCode::BAD_REQUEST,
            message: format!("failed to read torrent file: {}", err),
        })?;
        file_name = Some(filename);
        file_bytes = Some(bytes.to_vec());
        break;
    }

    let filename = file_name.ok_or_else(|| ApiError {
        status: StatusCode::BAD_REQUEST,
        message: "missing torrent file".to_string(),
    })?;
    if !filename.to_lowercase().ends_with(".torrent") {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "invalid torrent filename".to_string(),
        });
    }

    let save_path = state
        .qbittorrent_download_dir
        .as_ref()
        .and_then(|path| path.to_str());
    let bytes = file_bytes.unwrap_or_default();
    if bytes.len() > MAX_TORRENT_BYTES {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "torrent file too large".to_string(),
        });
    }
    client
        .add_torrent_bytes(&filename, bytes, save_path)
        .await
        .map_err(map_qbittorrent_error)?;

    Ok((
        StatusCode::ACCEPTED,
        Json(QbittorrentAddResponse {
            status: "queued".to_string(),
        }),
    ))
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

    let role_level = normalize_role_level(claims.role_level);

    Ok(AuthContext {
        user_id: claims.sub,
        role_level,
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

async fn in_flight_middleware(
    State(state): State<Arc<AppState>>,
    request: Request<axum::body::Body>,
    next: middleware::Next,
) -> Response {
    let _guard = InFlightGuard::new(state.in_flight.clone());
    next.run(request).await
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
            let options = AutoMatchOptions::default();
            if let Err(err) = auto_match_unmatched(&state.db, &state.bangumi, options).await {
                tracing::warn!("auto match after index failed: {}", err);
            }
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
    role_level: i32,
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
            role_level INTEGER NOT NULL DEFAULT 1,\
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()\
        )",
    )
    .execute(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to init db: {}", err),
    })?;

    sqlx::query(
        "ALTER TABLE users ADD COLUMN IF NOT EXISTS role_level INTEGER NOT NULL DEFAULT 1",
    )
    .execute(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to migrate users: {}", err),
    })?;

    sqlx::query(
        "UPDATE users SET role_level = 3 WHERE role = 'admin' AND role_level < 3",
    )
    .execute(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to normalize user roles: {}", err),
    })?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS resource_submissions (\
            id BIGSERIAL PRIMARY KEY,\
            submitter_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,\
            kind TEXT NOT NULL,\
            magnet TEXT,\
            torrent_name TEXT,\
            torrent_bytes BYTEA,\
            dedup_hash TEXT,\
            status TEXT NOT NULL DEFAULT 'pending',\
            note TEXT,\
            decision_note TEXT,\
            decided_at TIMESTAMPTZ,\
            decided_by TEXT REFERENCES users(id),\
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),\
            CONSTRAINT resource_submissions_kind_chk CHECK (kind IN ('magnet','torrent')),\
            CONSTRAINT resource_submissions_status_chk CHECK (status IN ('pending','approved','rejected')),\
            CONSTRAINT resource_submissions_payload_chk CHECK (\
                (kind = 'magnet' AND magnet IS NOT NULL AND torrent_bytes IS NULL) \
                OR (kind = 'torrent' AND torrent_bytes IS NOT NULL)\
            )\
        )",
    )
    .execute(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to init resource submissions: {}", err),
    })?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS resource_submissions_status_idx \
         ON resource_submissions (status, created_at DESC)",
    )
    .execute(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to init resource submissions index: {}", err),
    })?;

    sqlx::query(
        "ALTER TABLE resource_submissions ADD COLUMN IF NOT EXISTS dedup_hash TEXT",
    )
    .execute(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to migrate resource submissions: {}", err),
    })?;

    sqlx::query(
        "ALTER TABLE resource_submissions ADD COLUMN IF NOT EXISTS decision_note TEXT",
    )
    .execute(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to migrate resource submissions decisions: {}", err),
    })?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS resource_submissions_dedup_idx \
         ON resource_submissions (dedup_hash)",
    )
    .execute(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to init resource submissions dedup: {}", err),
    })?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS user_settings (\
            user_id TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,\
            display_name TEXT,\
            theme TEXT,\
            playback_speed DOUBLE PRECISION,\
            subtitle_lang TEXT,\
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),\
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()\
        )",
    )
    .execute(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to init user settings: {}", err),
    })?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS media_progress (\
            user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,\
            media_id TEXT NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,\
            position_secs DOUBLE PRECISION NOT NULL,\
            duration_secs DOUBLE PRECISION,\
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),\
            PRIMARY KEY (user_id, media_id)\
        )",
    )
    .execute(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to init media progress: {}", err),
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
        "INSERT INTO users (id, password_hash, role, role_level) VALUES ($1, $2, 'admin', $3) \
        ON CONFLICT (id) DO NOTHING",
    )
    .bind(&auth.admin_user)
    .bind(&hash)
    .bind(ROLE_LEVEL_SUPER_ADMIN)
    .execute(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to ensure admin user: {}", err),
    })?;

    sqlx::query(
        "UPDATE users SET role = 'admin', role_level = $1 WHERE id = $2",
    )
    .bind(ROLE_LEVEL_SUPER_ADMIN)
    .bind(&auth.admin_user)
    .execute(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to elevate admin user: {}", err),
    })?;

    Ok(())
}

struct DbUser {
    user_id: String,
    password_hash: String,
    role: UserRole,
    role_level: i32,
}

async fn fetch_user(db: &PgPool, user_id: &str) -> Result<DbUser, ApiError> {
    let row = sqlx::query_as::<_, (String, String, i32)>(
        "SELECT id, password_hash, role_level FROM users WHERE id = $1",
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

    let role_level = normalize_role_level(row.2);
    let role = role_from_level(role_level);

    Ok(DbUser {
        user_id: row.0,
        password_hash: row.1,
        role,
        role_level,
    })
}

async fn create_user(
    db: &PgPool,
    user_id: &str,
    password_hash: &str,
    role_level: i32,
) -> Result<DbUser, ApiError> {
    let role_level = normalize_role_level(role_level);
    let role = role_from_level(role_level);
    let role_str = role_to_str(role);
    let row = sqlx::query_as::<_, (String, String, i32)>(
        "INSERT INTO users (id, password_hash, role, role_level) VALUES ($1, $2, $3, $4) \
        RETURNING id, password_hash, role_level",
    )
    .bind(user_id)
    .bind(password_hash)
    .bind(role_str)
    .bind(role_level)
    .fetch_one(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::BAD_REQUEST,
        message: format!("failed to create user: {}", err),
    })?;

    let role_level = normalize_role_level(row.2);
    let role = role_from_level(role_level);

    Ok(DbUser {
        user_id: row.0,
        password_hash: row.1,
        role,
        role_level,
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

async fn list_users(db: &PgPool) -> Result<Vec<UserSummary>, ApiError> {
    let rows = sqlx::query_as::<_, (String, i32, String)>(
        "SELECT id, role_level, created_at::text FROM users ORDER BY id",
    )
    .fetch_all(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to list users: {}", err),
    })?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let role_level = normalize_role_level(row.1);
            UserSummary {
                user_id: row.0,
                role: role_from_level(role_level),
                role_level,
                created_at: row.2,
            }
        })
        .collect())
}

async fn update_user_role(
    db: &PgPool,
    user_id: &str,
    role_level: i32,
) -> Result<DbUser, ApiError> {
    let role_level = normalize_role_level(role_level);
    let role = role_from_level(role_level);
    let role_str = role_to_str(role);

    let row = sqlx::query_as::<_, (String, String, i32)>(
        "UPDATE users SET role = $2, role_level = $3 WHERE id = $1 \
        RETURNING id, password_hash, role_level",
    )
    .bind(user_id)
    .bind(role_str)
    .bind(role_level)
    .fetch_optional(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to update user role: {}", err),
    })?
    .ok_or_else(|| ApiError {
        status: StatusCode::NOT_FOUND,
        message: "user not found".to_string(),
    })?;

    let role_level = normalize_role_level(row.2);
    let role = role_from_level(role_level);

    Ok(DbUser {
        user_id: row.0,
        password_hash: row.1,
        role,
        role_level,
    })
}

#[derive(Debug, FromRow)]
struct CollectionFetchRow {
    kind: String,
    status: String,
    magnet: Option<String>,
    torrent_name: Option<String>,
    torrent_bytes: Option<Vec<u8>>,
}

#[derive(Debug, FromRow)]
struct CollectionApproveRow {
    id: i64,
    status: String,
}

#[derive(Debug, FromRow)]
struct CollectionListRow {
    id: i64,
    submitter_id: String,
    kind: String,
    status: String,
    magnet: Option<String>,
    torrent_name: Option<String>,
    note: Option<String>,
    decision_note: Option<String>,
    created_at: String,
    decided_at: Option<String>,
    decided_by: Option<String>,
}

#[derive(Debug, FromRow)]
struct UserSettingsRow {
    display_name: Option<String>,
    theme: Option<String>,
    playback_speed: Option<f64>,
    subtitle_lang: Option<String>,
}

#[derive(Debug, FromRow)]
struct MediaProgressRow {
    position_secs: f64,
    duration_secs: Option<f64>,
}

#[derive(Debug, FromRow)]
struct MediaEntryRow {
    id: String,
    filename: String,
    size: i64,
    path: String,
}

#[derive(Debug, FromRow)]
struct MediaParseRowDb {
    title: Option<String>,
    episode: Option<String>,
    season: Option<String>,
    year: Option<String>,
    release_group: Option<String>,
    resolution: Option<String>,
}

#[derive(Debug, FromRow)]
struct MediaMatchRowDb {
    subject_id: i64,
    episode_id: Option<i64>,
    method: String,
    confidence: Option<f32>,
    reason: Option<String>,
}

#[derive(Debug, FromRow)]
struct BangumiSubjectRow {
    id: i64,
    name: String,
    name_cn: String,
    air_date: Option<String>,
    total_episodes: Option<i64>,
}

#[derive(Debug, FromRow)]
struct BangumiEpisodeRow {
    id: i64,
    sort: f64,
    ep: Option<f64>,
    name: String,
    name_cn: String,
    air_date: Option<String>,
}

#[derive(Debug, FromRow)]
struct JobQueueRow {
    id: i64,
    job_type: String,
    status: String,
    attempts: i32,
    max_attempts: i32,
    payload: Value,
    result: Option<Value>,
    last_error: Option<String>,
    scheduled_at: String,
    locked_at: Option<String>,
    locked_by: Option<String>,
    created_at: String,
    updated_at: String,
}

async fn list_collection_items(
    db: &PgPool,
    auth: &AuthContext,
    status: Option<&str>,
) -> Result<Vec<CollectionItem>, ApiError> {
    let rows = if auth.role_level >= ROLE_LEVEL_ADMIN {
        sqlx::query_as::<_, CollectionListRow>(
            "SELECT id, submitter_id, kind, status, magnet, torrent_name, note, decision_note, \
             created_at::text, decided_at::text, decided_by \
             FROM resource_submissions \
             WHERE ($1::text IS NULL OR status = $1) \
             ORDER BY created_at DESC",
        )
        .bind(status)
        .fetch_all(db)
        .await
    } else {
        sqlx::query_as::<_, CollectionListRow>(
            "SELECT id, submitter_id, kind, status, magnet, torrent_name, note, decision_note, \
             created_at::text, decided_at::text, decided_by \
             FROM resource_submissions \
             WHERE submitter_id = $1 AND ($2::text IS NULL OR status = $2) \
             ORDER BY created_at DESC",
        )
        .bind(&auth.user_id)
        .bind(status)
        .fetch_all(db)
        .await
    }
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to list submissions: {}", err),
    })?;

    Ok(rows
        .into_iter()
        .map(|row| CollectionItem {
            id: row.id,
            submitter_id: row.submitter_id,
            kind: row.kind,
            status: row.status,
            magnet: row.magnet,
            torrent_name: row.torrent_name,
            note: row.note,
            decision_note: row.decision_note,
            created_at: row.created_at,
            decided_at: row.decided_at,
            decided_by: row.decided_by,
        })
        .collect())
}

async fn create_collection_magnet(
    db: &PgPool,
    submitter_id: &str,
    magnet: &str,
    note: Option<String>,
) -> Result<i64, ApiError> {
    let magnet = magnet.trim();
    if magnet.is_empty() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "magnet is empty".to_string(),
        });
    }
    if !magnet.starts_with("magnet:") {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "invalid magnet link".to_string(),
        });
    }

    let dedup_hash = sha256_hex(magnet.as_bytes());
    if submission_exists(db, &dedup_hash).await? {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            message: "submission already exists".to_string(),
        });
    }

    let row = sqlx::query_as::<_, (i64,)>(
        "INSERT INTO resource_submissions (submitter_id, kind, magnet, note, dedup_hash) \
         VALUES ($1, 'magnet', $2, $3, $4) RETURNING id",
    )
    .bind(submitter_id)
    .bind(magnet)
    .bind(note)
    .bind(dedup_hash)
    .fetch_one(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::BAD_REQUEST,
        message: format!("failed to store submission: {}", err),
    })?;

    Ok(row.0)
}

async fn create_collection_torrent(
    db: &PgPool,
    submitter_id: &str,
    filename: &str,
    bytes: Vec<u8>,
    note: Option<String>,
) -> Result<i64, ApiError> {
    if bytes.is_empty() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "torrent file is empty".to_string(),
        });
    }

    let dedup_hash = sha256_hex(&bytes);
    if submission_exists(db, &dedup_hash).await? {
        return Err(ApiError {
            status: StatusCode::CONFLICT,
            message: "submission already exists".to_string(),
        });
    }

    let row = sqlx::query_as::<_, (i64,)>(
        "INSERT INTO resource_submissions (submitter_id, kind, torrent_name, torrent_bytes, note, dedup_hash) \
         VALUES ($1, 'torrent', $2, $3, $4, $5) RETURNING id",
    )
    .bind(submitter_id)
    .bind(filename)
    .bind(bytes)
    .bind(note)
    .bind(dedup_hash)
    .fetch_one(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::BAD_REQUEST,
        message: format!("failed to store submission: {}", err),
    })?;

    Ok(row.0)
}

async fn fetch_collection_item(db: &PgPool, id: i64) -> Result<CollectionFetchRow, ApiError> {
    sqlx::query_as::<_, CollectionFetchRow>(
        "SELECT kind, status, magnet, torrent_name, torrent_bytes \
         FROM resource_submissions WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to fetch submission: {}", err),
    })?
    .ok_or_else(|| ApiError {
        status: StatusCode::NOT_FOUND,
        message: "submission not found".to_string(),
    })
}

async fn approve_collection_item(
    db: &PgPool,
    id: i64,
    approver_id: &str,
    decision_note: Option<String>,
) -> Result<CollectionApproveRow, ApiError> {
    let row = sqlx::query_as::<_, CollectionApproveRow>(
        "UPDATE resource_submissions \
         SET status = 'approved', decided_at = NOW(), decided_by = $2, decision_note = $3 \
         WHERE id = $1 \
         RETURNING id, status",
    )
    .bind(id)
    .bind(approver_id)
    .bind(decision_note)
    .fetch_optional(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to approve submission: {}", err),
    })?
    .ok_or_else(|| ApiError {
        status: StatusCode::NOT_FOUND,
        message: "submission not found".to_string(),
    })?;

    Ok(row)
}

async fn reject_collection_item(
    db: &PgPool,
    id: i64,
    approver_id: &str,
    decision_note: Option<String>,
) -> Result<CollectionApproveRow, ApiError> {
    let row = sqlx::query_as::<_, CollectionApproveRow>(
        "UPDATE resource_submissions \
         SET status = 'rejected', decided_at = NOW(), decided_by = $2, decision_note = $3 \
         WHERE id = $1 \
         RETURNING id, status",
    )
    .bind(id)
    .bind(approver_id)
    .bind(decision_note)
    .fetch_optional(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to reject submission: {}", err),
    })?
    .ok_or_else(|| ApiError {
        status: StatusCode::NOT_FOUND,
        message: "submission not found".to_string(),
    })?;

    Ok(row)
}

async fn delete_collection_item(
    db: &PgPool,
    auth: &AuthContext,
    id: i64,
) -> Result<(), ApiError> {
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT submitter_id, status FROM resource_submissions WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to fetch submission: {}", err),
    })?
    .ok_or_else(|| ApiError {
        status: StatusCode::NOT_FOUND,
        message: "submission not found".to_string(),
    })?;

    if auth.role_level < ROLE_LEVEL_ADMIN {
        if row.0 != auth.user_id {
            return Err(not_found_error());
        }
        if row.1 != "pending" {
            return Err(ApiError {
                status: StatusCode::CONFLICT,
                message: "submission already processed".to_string(),
            });
        }
    }

    sqlx::query("DELETE FROM resource_submissions WHERE id = $1")
        .bind(id)
        .execute(db)
        .await
        .map_err(|err| ApiError {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: format!("failed to delete submission: {}", err),
        })?;

    Ok(())
}

async fn submission_exists(db: &PgPool, dedup_hash: &str) -> Result<bool, ApiError> {
    let row = sqlx::query_scalar::<_, i64>(
        "SELECT 1 FROM resource_submissions \
         WHERE dedup_hash = $1 AND status IN ('pending', 'approved') LIMIT 1",
    )
    .bind(dedup_hash)
    .fetch_optional(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to check submissions: {}", err),
    })?;
    Ok(row.is_some())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(&mut out, "{:02x}", byte);
    }
    out
}

fn default_user_settings() -> UserSettingsResponse {
    UserSettingsResponse {
        display_name: None,
        theme: "dark".to_string(),
        playback_speed: 1.0,
        subtitle_lang: None,
    }
}

async fn fetch_user_settings(
    db: &PgPool,
    user_id: &str,
) -> Result<UserSettingsResponse, ApiError> {
    let row = sqlx::query_as::<_, UserSettingsRow>(
        "SELECT display_name, theme, playback_speed, subtitle_lang \
         FROM user_settings WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to fetch settings: {}", err),
    })?;

    let defaults = default_user_settings();
    if let Some(row) = row {
        Ok(UserSettingsResponse {
            display_name: row.display_name,
            theme: row.theme.unwrap_or(defaults.theme),
            playback_speed: row.playback_speed.unwrap_or(defaults.playback_speed),
            subtitle_lang: row.subtitle_lang,
        })
    } else {
        Ok(defaults)
    }
}

async fn upsert_user_settings(
    db: &PgPool,
    user_id: &str,
    payload: UpdateSettingsRequest,
) -> Result<UserSettingsResponse, ApiError> {
    let current = fetch_user_settings(db, user_id).await?;
    let playback_speed = payload.playback_speed.unwrap_or(current.playback_speed);
    if playback_speed <= 0.0 {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "playback_speed must be positive".to_string(),
        });
    }

    let theme = payload
        .theme
        .clone()
        .unwrap_or_else(|| current.theme.clone());
    if theme.trim().is_empty() {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "theme must not be empty".to_string(),
        });
    }

    let display_name = payload.display_name.or(current.display_name);
    let subtitle_lang = payload.subtitle_lang.or(current.subtitle_lang);

    sqlx::query(
        "INSERT INTO user_settings (user_id, display_name, theme, playback_speed, subtitle_lang) \
         VALUES ($1, $2, $3, $4, $5) \
         ON CONFLICT (user_id) DO UPDATE SET \
            display_name = EXCLUDED.display_name, \
            theme = EXCLUDED.theme, \
            playback_speed = EXCLUDED.playback_speed, \
            subtitle_lang = EXCLUDED.subtitle_lang, \
            updated_at = NOW()",
    )
    .bind(user_id)
    .bind(&display_name)
    .bind(&theme)
    .bind(playback_speed)
    .bind(&subtitle_lang)
    .execute(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to update settings: {}", err),
    })?;

    Ok(UserSettingsResponse {
        display_name,
        theme,
        playback_speed,
        subtitle_lang,
    })
}

async fn fetch_media_progress(
    db: &PgPool,
    user_id: &str,
    media_id: &str,
) -> Result<MediaProgressResponse, ApiError> {
    let row = sqlx::query_as::<_, MediaProgressRow>(
        "SELECT position_secs, duration_secs \
         FROM media_progress WHERE user_id = $1 AND media_id = $2",
    )
    .bind(user_id)
    .bind(media_id)
    .fetch_optional(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to fetch progress: {}", err),
    })?;

    Ok(MediaProgressResponse {
        media_id: media_id.to_string(),
        position_secs: row.as_ref().map(|r| r.position_secs).unwrap_or(0.0),
        duration_secs: row.and_then(|r| r.duration_secs),
    })
}

async fn list_media_progress(
    db: &PgPool,
    user_id: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<MediaProgressItem>, ApiError> {
    let rows = sqlx::query_as::<_, (String, String, f64, Option<f64>, String)>(
        "SELECT p.media_id, f.filename, p.position_secs, p.duration_secs, p.updated_at::text \
         FROM media_progress p \
         JOIN media_files f ON f.id = p.media_id \
         WHERE p.user_id = $1 \
         ORDER BY p.updated_at DESC \
         LIMIT $2 OFFSET $3",
    )
    .bind(user_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to list progress: {}", err),
    })?;

    Ok(rows
        .into_iter()
        .map(|row| MediaProgressItem {
            media_id: row.0,
            filename: row.1,
            position_secs: row.2,
            duration_secs: row.3,
            updated_at: row.4,
        })
        .collect())
}

async fn upsert_media_progress(
    db: &PgPool,
    user_id: &str,
    media_id: &str,
    payload: UpdateProgressRequest,
) -> Result<MediaProgressResponse, ApiError> {
    if payload.position_secs < 0.0 {
        return Err(ApiError {
            status: StatusCode::BAD_REQUEST,
            message: "position_secs must be >= 0".to_string(),
        });
    }
    if let Some(duration) = payload.duration_secs {
        if duration < 0.0 {
            return Err(ApiError {
                status: StatusCode::BAD_REQUEST,
                message: "duration_secs must be >= 0".to_string(),
            });
        }
    }

    let row = sqlx::query_as::<_, MediaProgressRow>(
        "INSERT INTO media_progress (user_id, media_id, position_secs, duration_secs) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (user_id, media_id) DO UPDATE SET \
            position_secs = EXCLUDED.position_secs, \
            duration_secs = EXCLUDED.duration_secs, \
            updated_at = NOW() \
         RETURNING position_secs, duration_secs",
    )
    .bind(user_id)
    .bind(media_id)
    .bind(payload.position_secs)
    .bind(payload.duration_secs)
    .fetch_one(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to update progress: {}", err),
    })?;

    Ok(MediaProgressResponse {
        media_id: media_id.to_string(),
        position_secs: row.position_secs,
        duration_secs: row.duration_secs,
    })
}

async fn fetch_media_entry(db: &PgPool, media_id: &str) -> Result<MediaEntry, ApiError> {
    let row = sqlx::query_as::<_, MediaEntryRow>(
        "SELECT id, filename, size, path FROM media_files WHERE id = $1",
    )
    .bind(media_id)
    .fetch_optional(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to fetch media: {}", err),
    })?
    .ok_or_else(|| ApiError {
        status: StatusCode::NOT_FOUND,
        message: "media not found".to_string(),
    })?;

    Ok(MediaEntry {
        id: row.id,
        filename: row.filename,
        size: row.size as u64,
        path: PathBuf::from(row.path),
    })
}

async fn fetch_media_parse(
    db: &PgPool,
    media_id: &str,
) -> Result<Option<MediaParseInfo>, ApiError> {
    let row = sqlx::query_as::<_, MediaParseRowDb>(
        "SELECT title, episode, season, year, release_group, resolution \
         FROM media_parses WHERE media_id = $1",
    )
    .bind(media_id)
    .fetch_optional(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to fetch parse: {}", err),
    })?;

    Ok(row.map(|row| MediaParseInfo {
        title: row.title,
        episode: row.episode,
        season: row.season,
        year: row.year,
        release_group: row.release_group,
        resolution: row.resolution,
    }))
}

async fn fetch_media_match(
    db: &PgPool,
    media_id: &str,
) -> Result<Option<MediaMatchRowDb>, ApiError> {
    let row = sqlx::query_as::<_, MediaMatchRowDb>(
        "SELECT subject_id, episode_id, method, confidence, reason \
         FROM media_matches WHERE media_id = $1",
    )
    .bind(media_id)
    .fetch_optional(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to fetch match: {}", err),
    })?;

    Ok(row)
}

async fn fetch_bangumi_subject(
    db: &PgPool,
    subject_id: i64,
) -> Result<BangumiSubjectInfo, ApiError> {
    let row = sqlx::query_as::<_, BangumiSubjectRow>(
        "SELECT id, name, name_cn, air_date, total_episodes \
         FROM bangumi_subjects WHERE id = $1",
    )
    .bind(subject_id)
    .fetch_optional(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to fetch subject: {}", err),
    })?
    .ok_or_else(|| ApiError {
        status: StatusCode::NOT_FOUND,
        message: "subject not found".to_string(),
    })?;

    Ok(BangumiSubjectInfo {
        id: row.id,
        name: row.name,
        name_cn: row.name_cn,
        air_date: row.air_date,
        total_episodes: row.total_episodes,
    })
}

async fn fetch_bangumi_episode(
    db: &PgPool,
    episode_id: i64,
) -> Result<Option<BangumiEpisodeInfo>, ApiError> {
    let row = sqlx::query_as::<_, BangumiEpisodeRow>(
        "SELECT id, sort, ep, name, name_cn, air_date \
         FROM bangumi_episodes WHERE id = $1",
    )
    .bind(episode_id)
    .fetch_optional(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to fetch episode: {}", err),
    })?;

    Ok(row.map(|row| BangumiEpisodeInfo {
        id: row.id,
        sort: row.sort,
        ep: row.ep,
        name: row.name,
        name_cn: row.name_cn,
        air_date: row.air_date,
    }))
}

async fn list_bangumi_episodes(
    db: &PgPool,
    subject_id: i64,
) -> Result<Vec<BangumiEpisodeInfo>, ApiError> {
    let rows = sqlx::query_as::<_, BangumiEpisodeRow>(
        "SELECT id, sort, ep, name, name_cn, air_date \
         FROM bangumi_episodes WHERE subject_id = $1 ORDER BY sort",
    )
    .bind(subject_id)
    .fetch_all(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to list episodes: {}", err),
    })?;

    Ok(rows
        .into_iter()
        .map(|row| BangumiEpisodeInfo {
            id: row.id,
            sort: row.sort,
            ep: row.ep,
            name: row.name,
            name_cn: row.name_cn,
            air_date: row.air_date,
        })
        .collect())
}

async fn fetch_media_for_episode(
    db: &PgPool,
    subject_id: i64,
    episode_id: i64,
) -> Result<Option<NextMediaEntry>, ApiError> {
    let row = sqlx::query_as::<_, (String, String, i64)>(
        "SELECT f.id, f.filename, f.size \
         FROM media_matches m \
         JOIN media_files f ON f.id = m.media_id \
         WHERE m.subject_id = $1 AND m.episode_id = $2 \
         ORDER BY f.filename \
         LIMIT 1",
    )
    .bind(subject_id)
    .bind(episode_id)
    .fetch_optional(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to fetch next media: {}", err),
    })?;

    Ok(row.map(|row| NextMediaEntry {
        id: row.0,
        filename: row.1,
        size: row.2 as u64,
    }))
}

async fn maybe_sync_episodes(state: &AppState, subject_id: i64) -> Result<(), ApiError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(1) FROM bangumi_episodes WHERE subject_id = $1",
    )
    .bind(subject_id)
    .fetch_one(&state.db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to check episodes: {}", err),
    })?;

    if count == 0 {
        sync_bangumi_subject(&state.db, &state.bangumi, subject_id)
            .await
            .map_err(|err| ApiError {
                status: StatusCode::BAD_GATEWAY,
                message: format!("failed to sync bangumi: {}", err),
            })?;
    }

    Ok(())
}

async fn fetch_media_stats(db: &PgPool) -> Result<(i64, i64), ApiError> {
    let row = sqlx::query_as::<_, (i64, i64)>(
        "SELECT COUNT(*)::BIGINT, COALESCE(SUM(size), 0)::BIGINT FROM media_files",
    )
    .fetch_one(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to fetch media stats: {}", err),
    })?;
    Ok(row)
}

async fn fetch_job_counts(db: &PgPool) -> Result<JobCounts, ApiError> {
    let rows = sqlx::query_as::<_, (String, i64)>(
        "SELECT status, COUNT(*) FROM job_queue GROUP BY status",
    )
    .fetch_all(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to fetch job counts: {}", err),
    })?;

    let mut counts = JobCounts {
        queued: 0,
        running: 0,
        retry: 0,
        done: 0,
        failed: 0,
    };
    for (status, count) in rows {
        match status.as_str() {
            "queued" => counts.queued = count,
            "running" => counts.running = count,
            "retry" => counts.retry = count,
            "done" => counts.done = count,
            "failed" => counts.failed = count,
            _ => {}
        }
    }
    Ok(counts)
}

async fn list_job_queue(
    db: &PgPool,
    status: Option<&str>,
    limit: i64,
    offset: i64,
) -> Result<Vec<JobQueueItem>, ApiError> {
    let rows = sqlx::query_as::<_, JobQueueRow>(
        "SELECT id, job_type, status, attempts, max_attempts, payload, result, last_error, \
         scheduled_at::text, locked_at::text, locked_by, created_at::text, updated_at::text \
         FROM job_queue \
         WHERE ($1::text IS NULL OR status = $1) \
         ORDER BY id DESC LIMIT $2 OFFSET $3",
    )
    .bind(status)
    .bind(limit)
    .bind(offset)
    .fetch_all(db)
    .await
    .map_err(|err| ApiError {
        status: StatusCode::INTERNAL_SERVER_ERROR,
        message: format!("failed to list job queue: {}", err),
    })?;

    Ok(rows
        .into_iter()
        .map(|row| JobQueueItem {
            id: row.id,
            job_type: row.job_type,
            status: row.status,
            attempts: row.attempts,
            max_attempts: row.max_attempts,
            payload: row.payload,
            result: row.result,
            last_error: row.last_error,
            scheduled_at: row.scheduled_at,
            locked_at: row.locked_at,
            locked_by: row.locked_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
        .collect())
}

fn collect_system_metrics() -> SystemMetrics {
    let mut sys = System::new();
    sys.refresh_memory();
    sys.refresh_cpu();
    sys.refresh_processes();

    let total_memory_bytes = sys.total_memory().saturating_mul(1024);
    let used_memory_bytes = sys.used_memory().saturating_mul(1024);
    let cpu_usage_percent = sys.global_cpu_info().cpu_usage();

    let process_memory_bytes = sysinfo::get_current_pid()
        .ok()
        .and_then(|pid| sys.process(pid))
        .map(|process| process.memory().saturating_mul(1024))
        .unwrap_or(0);

    SystemMetrics {
        total_memory_bytes,
        used_memory_bytes,
        process_memory_bytes,
        cpu_usage_percent,
    }
}

fn collect_storage_metrics(state: &AppState) -> StorageMetrics {
    let mut disks = Disks::new_with_refreshed_list();
    disks.refresh();

    let media_dir = disk_usage_for_path(&state.config.media_dir, &disks);
    let cache_dir = disk_usage_for_path(&state.config.cache_dir, &disks);
    let qbittorrent_download_dir = state
        .qbittorrent_download_dir
        .as_ref()
        .and_then(|path| disk_usage_for_path(path, &disks));

    StorageMetrics {
        media_dir,
        cache_dir,
        qbittorrent_download_dir,
    }
}

async fn collect_network_metrics(state: &AppState) -> NetworkMetrics {
    let (rx_bytes, tx_bytes, mut interfaces) = snapshot_network_totals();
    interfaces.sort_by(|left, right| left.name.cmp(&right.name));

    let now = Instant::now();
    let mut guard = state.metrics_state.lock().await;
    let (rx_rate, tx_rate) = if let Some(snapshot) = &guard.last_network {
        let elapsed = now.duration_since(snapshot.at).as_secs_f64();
        if elapsed > 0.0 {
            let rx_delta = rx_bytes.saturating_sub(snapshot.rx_bytes) as f64;
            let tx_delta = tx_bytes.saturating_sub(snapshot.tx_bytes) as f64;
            (rx_delta / elapsed, tx_delta / elapsed)
        } else {
            (0.0, 0.0)
        }
    } else {
        (0.0, 0.0)
    };
    guard.last_network = Some(NetworkSnapshot {
        rx_bytes,
        tx_bytes,
        at: now,
    });

    NetworkMetrics {
        rx_bytes,
        tx_bytes,
        rx_bytes_per_sec: rx_rate,
        tx_bytes_per_sec: tx_rate,
        interfaces,
    }
}

async fn collect_qbittorrent_metrics(state: &AppState) -> Option<QbittorrentTransferMetrics> {
    let client = state.qbittorrent.as_ref()?;
    match client.transfer_info().await {
        Ok(info) => Some(QbittorrentTransferMetrics {
            download_speed_bytes: info.download_speed_bytes,
            upload_speed_bytes: info.upload_speed_bytes,
            download_total_bytes: info.download_total_bytes,
            upload_total_bytes: info.upload_total_bytes,
            download_rate_limit: info.download_rate_limit,
            upload_rate_limit: info.upload_rate_limit,
            dht_nodes: info.dht_nodes,
            connection_status: info.connection_status,
        }),
        Err(err) => {
            info!(error = %err, "qbittorrent metrics unavailable");
            None
        }
    }
}

fn snapshot_network_totals() -> (u64, u64, Vec<NetworkInterfaceMetrics>) {
    let mut networks = Networks::new_with_refreshed_list();
    networks.refresh();

    let mut rx_bytes: u64 = 0;
    let mut tx_bytes: u64 = 0;
    let mut interfaces = Vec::new();

    for (name, data) in &networks {
        let rx = data.total_received();
        let tx = data.total_transmitted();
        rx_bytes = rx_bytes.saturating_add(rx);
        tx_bytes = tx_bytes.saturating_add(tx);
        interfaces.push(NetworkInterfaceMetrics {
            name: name.clone(),
            rx_bytes: rx,
            tx_bytes: tx,
        });
    }

    (rx_bytes, tx_bytes, interfaces)
}

fn disk_usage_for_path(path: &StdPath, disks: &Disks) -> Option<DiskUsage> {
    let mut target = if path.is_absolute() {
        path.to_path_buf()
    } else if let Ok(cwd) = env::current_dir() {
        cwd.join(path)
    } else {
        path.to_path_buf()
    };
    if let Ok(real) = target.canonicalize() {
        target = real;
    }

    let mut best: Option<(&sysinfo::Disk, usize)> = None;
    for disk in disks.list() {
        let mount = disk.mount_point();
        if target.starts_with(mount) {
            let len = mount.as_os_str().len();
            if best.map_or(true, |(_, best_len)| len >= best_len) {
                best = Some((disk, len));
            }
        }
    }

    best.map(|(disk, _)| DiskUsage {
        mount_point: disk.mount_point().display().to_string(),
        total_bytes: disk.total_space(),
        available_bytes: disk.available_space(),
    })
}

fn role_to_str(role: UserRole) -> &'static str {
    match role {
        UserRole::Admin => "admin",
        UserRole::User => "user",
    }
}

fn map_qbittorrent_error(err: QbittorrentError) -> ApiError {
    let status = match err {
        QbittorrentError::InvalidInput(_) | QbittorrentError::Io(_) => StatusCode::BAD_REQUEST,
        QbittorrentError::AuthFailed(_) | QbittorrentError::Http(_) => StatusCode::BAD_GATEWAY,
        QbittorrentError::InvalidHeader(_) => StatusCode::INTERNAL_SERVER_ERROR,
    };
    ApiError {
        status,
        message: format!("qbittorrent error: {}", err),
    }
}

fn parse_episode_number(value: Option<&str>) -> Option<f64> {
    let text = value?.trim();
    if text.is_empty() {
        return None;
    }
    let mut current = String::new();
    for ch in text.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            current.push(ch);
        } else if !current.is_empty() {
            break;
        }
    }
    if current.is_empty() {
        return None;
    }
    current.parse::<f64>().ok()
}

fn find_episode_by_number(
    episodes: &[BangumiEpisodeInfo],
    value: f64,
) -> Option<BangumiEpisodeInfo> {
    let epsilon = 0.01;
    episodes
        .iter()
        .find(|ep| {
            ep.ep
                .map(|ep_no| (ep_no - value).abs() < epsilon)
                .unwrap_or_else(|| (ep.sort - value).abs() < epsilon)
        })
        .cloned()
}

fn find_next_episode(
    episodes: &[BangumiEpisodeInfo],
    current_sort: f64,
) -> Option<BangumiEpisodeInfo> {
    episodes
        .iter()
        .filter(|ep| ep.sort > current_sort + 0.0001)
        .min_by(|a, b| {
            a.sort
                .partial_cmp(&b.sort)
                .unwrap_or(CmpOrdering::Equal)
        })
        .cloned()
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

fn issue_token(auth: &AuthConfig, user_id: &str, role_level: i32) -> Result<String, ApiError> {
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
        role_level: normalize_role_level(role_level),
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
