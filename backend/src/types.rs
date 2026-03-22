use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use crate::bangumi::SearchFacets;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiEnvelope<T> {
    pub data: T,
}

impl<T> ApiEnvelope<T> {
    pub fn new(data: T) -> Self {
        Self { data }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorEnvelope {
    code: String,
    message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Upstream(String),
    #[error("{0}")]
    Internal(String),
}

impl AppError {
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::BadRequest(message.into())
    }

    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::Unauthorized(message.into())
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::NotFound(message.into())
    }

    pub fn upstream(message: impl Into<String>) -> Self {
        Self::Upstream(message.into())
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal(message.into())
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code) = match self {
            Self::BadRequest(_) => (StatusCode::BAD_REQUEST, "bad_request"),
            Self::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "unauthorized"),
            Self::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
            Self::Upstream(_) => (StatusCode::BAD_GATEWAY, "upstream_error"),
            Self::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
        };

        let body = Json(ErrorEnvelope {
            code: code.to_owned(),
            message: self.to_string(),
        });

        (status, body).into_response()
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthResponse {
    pub status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapResponse {
    pub device_id: String,
    pub viewer: ViewerSummary,
    pub admin_path: String,
    pub policy: PolicyDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewerSummary {
    pub kind: String,
    pub id: Option<i64>,
    pub label: String,
    pub device_id: Option<String>,
}

impl ViewerSummary {
    pub fn device(device_id: String) -> Self {
        Self {
            kind: "device".to_owned(),
            id: None,
            label: "当前设备".to_owned(),
            device_id: Some(device_id),
        }
    }

    pub fn user(id: i64, username: String) -> Self {
        Self {
            kind: "user".to_owned(),
            id: Some(id),
            label: username,
            device_id: None,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarResponse {
    pub days: Vec<CalendarDayDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarDayDto {
    pub weekday: WeekdayDto,
    pub items: Vec<SubjectCardDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeekdayDto {
    pub id: u8,
    pub cn: String,
    pub en: String,
    pub ja: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubjectCardDto {
    pub bangumi_subject_id: i64,
    pub title: String,
    pub title_cn: String,
    pub summary: String,
    pub release_status: String,
    pub air_date: Option<String>,
    pub broadcast_time: Option<String>,
    pub air_weekday: Option<u8>,
    pub image_portrait: Option<String>,
    pub image_banner: Option<String>,
    pub tags: Vec<String>,
    pub total_episodes: Option<i64>,
    pub rating_score: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchRequest {
    #[serde(default)]
    pub keyword: String,
    #[serde(default)]
    pub tag: Vec<String>,
    #[serde(default)]
    pub meta_tag: Vec<String>,
    #[serde(default)]
    pub sort: Option<String>,
    #[serde(default)]
    pub air_date_start: Option<String>,
    #[serde(default)]
    pub air_date_end: Option<String>,
    #[serde(default)]
    pub rating_min: Option<f64>,
    #[serde(default)]
    pub rating_max: Option<f64>,
    #[serde(default)]
    pub rating_count_min: Option<u32>,
    #[serde(default)]
    pub rating_count_max: Option<u32>,
    #[serde(default)]
    pub rank_min: Option<u32>,
    #[serde(default)]
    pub rank_max: Option<u32>,
    #[serde(default)]
    pub nsfw_mode: Option<String>,
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub page_size: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub items: Vec<SubjectCardDto>,
    pub facets: SearchFacets,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    pub has_next_page: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubjectDetailResponse {
    pub subject: SubjectDetailDto,
    pub episodes: Vec<EpisodeDto>,
    pub subscription: SubscriptionStateDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubjectDetailDto {
    pub bangumi_subject_id: i64,
    pub title: String,
    pub title_cn: String,
    pub summary: String,
    pub air_date: Option<String>,
    pub broadcast_time: Option<String>,
    pub air_weekday: Option<u8>,
    pub total_episodes: Option<i64>,
    pub image_portrait: Option<String>,
    pub image_banner: Option<String>,
    pub tags: Vec<String>,
    pub infobox: Vec<InfoboxItemDto>,
    pub rating_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InfoboxItemDto {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeDto {
    pub bangumi_episode_id: i64,
    pub sort: f64,
    pub episode_number: Option<f64>,
    pub title: String,
    pub title_cn: String,
    pub airdate: Option<String>,
    pub duration_seconds: Option<i64>,
    pub is_available: bool,
    pub availability_note: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionStateDto {
    pub is_subscribed: bool,
    pub subscription_count: i64,
    pub threshold: i64,
    pub source: ViewerSummary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToggleSubscriptionResponse {
    pub bangumi_subject_id: i64,
    pub subscription: SubscriptionStateDto,
    pub download: DownloadDecisionDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadDecisionDto {
    pub demand_state: String,
    pub reason: String,
    pub job: Option<DownloadJobDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadJobDto {
    pub id: i64,
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
    pub selected_candidate_id: Option<i64>,
    pub selection_updated_at: Option<String>,
    pub last_search_run_id: Option<i64>,
    pub search_status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminDownloadQueueResponse {
    pub items: Vec<DownloadJobDto>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadExecutionDto {
    pub id: i64,
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
    pub created_at: String,
    pub updated_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub replaced_at: Option<String>,
    pub failed_at: Option<String>,
    pub last_indexed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadExecutionEventDto {
    pub id: i64,
    pub download_execution_id: i64,
    pub level: String,
    pub event_kind: String,
    pub message: String,
    pub downloaded_bytes: Option<i64>,
    pub uploaded_bytes: Option<i64>,
    pub download_rate_bytes: Option<i64>,
    pub upload_rate_bytes: Option<i64>,
    pub peer_count: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadExecutionDecisionDto {
    pub reason: String,
    pub execution: Option<DownloadExecutionDto>,
    pub replaced_execution_id: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminDownloadExecutionsResponse {
    pub download_job_id: i64,
    pub items: Vec<DownloadExecutionDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminDownloadExecutionEventsResponse {
    pub download_execution_id: i64,
    pub items: Vec<DownloadExecutionEventDto>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ForceDownloadResponse {
    pub bangumi_subject_id: i64,
    pub decision: DownloadDecisionDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivateDownloadResponse {
    pub download_job_id: i64,
    pub decision: DownloadExecutionDecisionDto,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceCandidateDto {
    pub id: i64,
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
    pub discovered_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminDownloadCandidatesResponse {
    pub download_job_id: i64,
    pub items: Vec<ResourceCandidateDto>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceLibraryRequest {
    #[serde(default)]
    pub keyword: Option<String>,
    #[serde(default)]
    pub page: Option<usize>,
    #[serde(default)]
    pub page_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceLibraryItemDto {
    pub id: i64,
    pub bangumi_subject_id: i64,
    pub download_job_id: i64,
    pub download_execution_id: i64,
    pub resource_candidate_id: i64,
    pub slot_key: String,
    pub source_title: String,
    pub source_fansub_name: Option<String>,
    pub execution_state: String,
    pub relative_path: String,
    pub absolute_path: String,
    pub file_name: String,
    pub file_ext: String,
    pub size_bytes: i64,
    pub episode_index: Option<f64>,
    pub episode_end_index: Option<f64>,
    pub is_collection: bool,
    pub status: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceLibraryResponse {
    pub items: Vec<ResourceLibraryItemDto>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
    pub has_next_page: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialsRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthResponse {
    pub token: String,
    pub viewer: ViewerSummary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminAuthResponse {
    pub token: String,
    pub admin_username: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyDto {
    pub subscription_threshold: i64,
    pub replacement_window_hours: i64,
    pub prefer_same_fansub: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FansubRuleDto {
    pub id: i64,
    pub fansub_name: String,
    pub locale_preference: String,
    pub priority: i64,
    pub is_blacklist: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminCountsDto {
    pub devices: i64,
    pub users: i64,
    pub subscriptions: i64,
    pub fansub_rules: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminDashboardResponse {
    pub admin_username: String,
    pub policy: PolicyDto,
    pub fansub_rules: Vec<FansubRuleDto>,
    pub counts: AdminCountsDto,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeHttpStatsDto {
    pub active_requests: u64,
    pub total_requests: u64,
    pub failed_requests: u64,
    pub incoming_bytes: u64,
    pub outgoing_bytes: u64,
    pub last_route: String,
    pub last_status: u16,
    pub last_latency_ms: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeOverviewDto {
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminRuntimeResponse {
    pub server_address: String,
    pub uptime_seconds: u64,
    pub uptime_label: String,
    pub log_dir: String,
    pub download_engine: String,
    pub http: RuntimeHttpStatsDto,
    pub runtime: RuntimeOverviewDto,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatePolicyRequest {
    pub subscription_threshold: i64,
    pub replacement_window_hours: i64,
    pub prefer_same_fansub: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertFansubRuleRequest {
    pub fansub_name: String,
    pub locale_preference: String,
    pub priority: i64,
    pub is_blacklist: bool,
}
