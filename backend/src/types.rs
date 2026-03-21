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
    pub air_date: Option<String>,
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
    pub year: Option<i32>,
    #[serde(default)]
    pub tag: Option<String>,
    #[serde(default)]
    pub sort: Option<String>,
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
