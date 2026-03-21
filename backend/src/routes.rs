use std::{cmp::Ordering, collections::HashSet};

use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::HeaderMap,
    routing::{get, post, put},
};
use sqlx::SqlitePool;
use tower_http::{cors::CorsLayer, trace::TraceLayer};

use crate::{
    auth::{AdminIdentity, ViewerIdentity, extract_admin_token, extract_device_id, extract_user_token},
    bangumi::{BangumiClient, SearchFacets},
    config::AppConfig,
    db,
    types::{
        AdminDashboardResponse, ApiEnvelope, AppError, AuthResponse, BootstrapResponse,
        CalendarDayDto, CalendarResponse, CredentialsRequest, FansubRuleDto, HealthResponse,
        SearchRequest, SearchResponse, SubjectDetailResponse, SubscriptionStateDto,
        ToggleSubscriptionResponse, UpdatePolicyRequest, UpsertFansubRuleRequest, ViewerSummary,
    },
};

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub pool: SqlitePool,
    pub bangumi: BangumiClient,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/public/bootstrap", get(bootstrap))
        .route("/api/public/calendar", get(calendar))
        .route("/api/public/search", get(search))
        .route("/api/public/subjects/{subject_id}", get(subject_detail))
        .route(
            "/api/public/subscriptions/{subject_id}/toggle",
            post(toggle_subscription),
        )
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/me", get(current_user))
        .route("/api/auth/logout", post(logout))
        .route("/api/admin/login", post(admin_login))
        .route("/api/admin/logout", post(admin_logout))
        .route("/api/admin/dashboard", get(admin_dashboard))
        .route("/api/admin/policy", put(update_policy))
        .route("/api/admin/fansub-rules", post(create_fansub_rule))
        .with_state(state)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

async fn health() -> Json<ApiEnvelope<HealthResponse>> {
    Json(ApiEnvelope::new(HealthResponse {
        status: "ok".to_owned(),
    }))
}

async fn bootstrap(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiEnvelope<BootstrapResponse>>, AppError> {
    let device_id = require_device_id(&headers)?;
    db::touch_device(&state.pool, &device_id).await?;

    let viewer = resolve_viewer(&state.pool, &headers, &device_id).await?;
    let viewer_summary = viewer_to_summary(&viewer);
    let policy = db::load_policy(&state.pool).await?;

    Ok(Json(ApiEnvelope::new(BootstrapResponse {
        device_id,
        viewer: viewer_summary,
        admin_path: "/admin".to_owned(),
        policy,
    })))
}

async fn calendar(
    State(state): State<AppState>,
) -> Result<Json<ApiEnvelope<CalendarResponse>>, AppError> {
    let days = state
        .bangumi
        .fetch_calendar()
        .await?
        .into_iter()
        .map(|day| CalendarDayDto {
            weekday: day.to_weekday(),
            items: day.items.into_iter().map(|item| item.to_card()).collect(),
        })
        .collect();

    Ok(Json(ApiEnvelope::new(CalendarResponse { days })))
}

async fn search(
    State(state): State<AppState>,
    Query(request): Query<SearchRequest>,
) -> Result<Json<ApiEnvelope<SearchResponse>>, AppError> {
    const SEARCH_WINDOW_PAGES: usize = 5;
    const SEARCH_REMOTE_PAGE_SIZE: usize = 20;

    let page = request.page.unwrap_or(1).max(1);
    let page_size = request.page_size.unwrap_or(20).clamp(1, 20);
    let tag_filter = request.tag.as_ref().map(|value| value.to_lowercase());
    let mut seen_ids = HashSet::new();
    let mut items = Vec::new();

    for page_index in 0..SEARCH_WINDOW_PAGES {
        let response = state
            .bangumi
            .search_subjects(request.keyword.trim(), page_index * SEARCH_REMOTE_PAGE_SIZE)
            .await?;

        let page_items = response
            .data
            .into_iter()
            .filter(|subject| seen_ids.insert(subject.id))
            .collect::<Vec<_>>();

        if page_items.is_empty() {
            break;
        }

        let page_count = page_items.len();
        items.extend(page_items);

        if page_count < SEARCH_REMOTE_PAGE_SIZE {
            break;
        }
    }

    items.retain(|subject| {
        request.year.is_none_or(|year| {
            subject
                .date
                .as_ref()
                .or(subject.air_date.as_ref())
                .and_then(|date| date.split('-').next())
                .and_then(|year_text| year_text.parse::<i32>().ok())
                == Some(year)
        })
    });

    items.retain(|subject| {
        tag_filter.as_ref().is_none_or(|tag| {
            subject
                .tags
                .iter()
                .any(|subject_tag| subject_tag.name.to_lowercase().contains(tag))
        })
    });

    match request.sort.as_deref() {
        Some("title") => items.sort_by(|left, right| {
            left.name_cn
                .cmp(&right.name_cn)
                .then_with(|| left.name.cmp(&right.name))
        }),
        _ => items.sort_by(|left, right| {
            let left_score = left.rating.as_ref().and_then(|rating| rating.score);
            let right_score = right.rating.as_ref().and_then(|rating| rating.score);

            right_score
                .partial_cmp(&left_score)
                .unwrap_or(Ordering::Equal)
                .then_with(|| left.name_cn.cmp(&right.name_cn))
                .then_with(|| left.name.cmp(&right.name))
        }),
    }

    let mut years = items
        .iter()
        .filter_map(|subject| {
            subject
                .date
                .as_ref()
                .or(subject.air_date.as_ref())
                .and_then(|date| date.split('-').next())
                .and_then(|year_text| year_text.parse::<i32>().ok())
        })
        .collect::<Vec<_>>();
    years.sort_unstable();
    years.dedup();

    let mut tags = items
        .iter()
        .flat_map(|subject| subject.tags.iter().map(|tag| tag.name.clone()))
        .collect::<Vec<_>>();
    tags.sort_unstable();
    tags.dedup();

    let total = items.len();
    let start = (page - 1) * page_size;
    let paged_items = if start >= total {
        Vec::new()
    } else {
        items.into_iter()
            .skip(start)
            .take(page_size)
            .map(|subject| subject.to_card())
            .collect()
    };

    Ok(Json(ApiEnvelope::new(SearchResponse {
        items: paged_items,
        facets: SearchFacets { years, tags },
        total,
        page,
        page_size,
        has_next_page: start + page_size < total,
    })))
}

async fn subject_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(subject_id): Path<i64>,
) -> Result<Json<ApiEnvelope<SubjectDetailResponse>>, AppError> {
    let device_id = extract_device_id(&headers);
    if let Some(id) = device_id.as_ref() {
        db::touch_device(&state.pool, id).await?;
    }

    let viewer = resolve_optional_viewer(&state.pool, &headers, device_id.as_deref()).await?;
    let policy = db::load_policy(&state.pool).await?;

    let (subject, episodes) = tokio::try_join!(
        state.bangumi.fetch_subject(subject_id),
        state.bangumi.fetch_episodes(subject_id)
    )?;

    let (is_subscribed, subscription_count) = if let Some(viewer) = viewer.as_ref() {
        db::subscription_state(&state.pool, viewer, subject_id).await?
    } else {
        (false, 0)
    };

    Ok(Json(ApiEnvelope::new(SubjectDetailResponse {
        subject: subject.to_detail(),
        episodes: episodes.into_iter().map(|episode| episode.to_dto()).collect(),
        subscription: SubscriptionStateDto {
            is_subscribed,
            subscription_count,
            threshold: policy.subscription_threshold,
            source: viewer
                .as_ref()
                .map(viewer_to_summary)
                .unwrap_or(ViewerSummary::device("guest-device".to_owned())),
        },
    })))
}

async fn toggle_subscription(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(subject_id): Path<i64>,
) -> Result<Json<ApiEnvelope<ToggleSubscriptionResponse>>, AppError> {
    let device_id = require_device_id(&headers)?;
    db::touch_device(&state.pool, &device_id).await?;

    let viewer = resolve_viewer(&state.pool, &headers, &device_id).await?;
    let policy = db::load_policy(&state.pool).await?;
    let (is_subscribed, subscription_count) =
        db::toggle_subscription(&state.pool, &viewer, subject_id).await?;

    Ok(Json(ApiEnvelope::new(ToggleSubscriptionResponse {
        bangumi_subject_id: subject_id,
        subscription: SubscriptionStateDto {
            is_subscribed,
            subscription_count,
            threshold: policy.subscription_threshold,
            source: viewer_to_summary(&viewer),
        },
    })))
}

async fn register(
    State(state): State<AppState>,
    Json(payload): Json<CredentialsRequest>,
) -> Result<Json<ApiEnvelope<AuthResponse>>, AppError> {
    validate_credentials(&payload.username, &payload.password)?;
    let (viewer, token) =
        db::register_user(&state.pool, &payload.username, &payload.password, &state.config.auth)
            .await?;

    Ok(Json(ApiEnvelope::new(AuthResponse {
        token,
        viewer: viewer_to_summary(&viewer),
    })))
}

async fn login(
    State(state): State<AppState>,
    Json(payload): Json<CredentialsRequest>,
) -> Result<Json<ApiEnvelope<AuthResponse>>, AppError> {
    validate_credentials(&payload.username, &payload.password)?;
    let (viewer, token) =
        db::login_user(&state.pool, &payload.username, &payload.password, &state.config.auth)
            .await?;

    Ok(Json(ApiEnvelope::new(AuthResponse {
        token,
        viewer: viewer_to_summary(&viewer),
    })))
}

async fn current_user(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiEnvelope<Option<ViewerSummary>>>, AppError> {
    let token = extract_user_token(&headers);
    let viewer = if let Some(token) = token {
        db::user_from_token(&state.pool, &token)
            .await?
            .map(|viewer| viewer_to_summary(&viewer))
    } else {
        None
    };

    Ok(Json(ApiEnvelope::new(viewer)))
}

async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiEnvelope<bool>>, AppError> {
    let Some(token) = extract_user_token(&headers) else {
        return Err(AppError::unauthorized("missing user token"));
    };

    db::logout_user(&state.pool, &token).await?;
    Ok(Json(ApiEnvelope::new(true)))
}

async fn admin_login(
    State(state): State<AppState>,
    Json(payload): Json<CredentialsRequest>,
) -> Result<Json<ApiEnvelope<crate::types::AdminAuthResponse>>, AppError> {
    validate_credentials(&payload.username, &payload.password)?;
    let (admin, token) =
        db::login_admin(&state.pool, &payload.username, &payload.password, &state.config.auth)
            .await?;

    Ok(Json(ApiEnvelope::new(crate::types::AdminAuthResponse {
        token,
        admin_username: admin.username,
    })))
}

async fn admin_logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiEnvelope<bool>>, AppError> {
    let Some(token) = extract_admin_token(&headers) else {
        return Err(AppError::unauthorized("missing admin token"));
    };

    db::logout_admin(&state.pool, &token).await?;
    Ok(Json(ApiEnvelope::new(true)))
}

async fn admin_dashboard(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiEnvelope<AdminDashboardResponse>>, AppError> {
    let admin = require_admin(&state.pool, &headers).await?;
    let policy = db::load_policy(&state.pool).await?;
    let fansub_rules = db::list_fansub_rules(&state.pool).await?;
    let counts = db::admin_counts(&state.pool).await?;

    Ok(Json(ApiEnvelope::new(AdminDashboardResponse {
        admin_username: admin.username,
        policy,
        fansub_rules,
        counts,
    })))
}

async fn update_policy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdatePolicyRequest>,
) -> Result<Json<ApiEnvelope<crate::types::PolicyDto>>, AppError> {
    require_admin(&state.pool, &headers).await?;

    let policy = db::update_policy(
        &state.pool,
        payload.subscription_threshold,
        payload.replacement_window_hours,
        payload.prefer_same_fansub,
    )
    .await?;

    Ok(Json(ApiEnvelope::new(policy)))
}

async fn create_fansub_rule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpsertFansubRuleRequest>,
) -> Result<Json<ApiEnvelope<FansubRuleDto>>, AppError> {
    require_admin(&state.pool, &headers).await?;

    let rule = db::add_fansub_rule(
        &state.pool,
        &payload.fansub_name,
        &payload.locale_preference,
        payload.priority,
        payload.is_blacklist,
    )
    .await?;

    Ok(Json(ApiEnvelope::new(rule)))
}

async fn resolve_viewer(
    pool: &SqlitePool,
    headers: &HeaderMap,
    device_id: &str,
) -> Result<ViewerIdentity, AppError> {
    resolve_optional_viewer(pool, headers, Some(device_id))
        .await?
        .ok_or_else(|| AppError::bad_request("missing device identity"))
}

async fn resolve_optional_viewer(
    pool: &SqlitePool,
    headers: &HeaderMap,
    fallback_device_id: Option<&str>,
) -> Result<Option<ViewerIdentity>, AppError> {
    if let Some(token) = extract_user_token(headers) {
        if let Some(viewer) = db::user_from_token(pool, &token).await? {
            return Ok(Some(viewer));
        }
    }

    Ok(fallback_device_id.map(|id| ViewerIdentity::Device {
        id: id.to_owned(),
    }))
}

async fn require_admin(pool: &SqlitePool, headers: &HeaderMap) -> Result<AdminIdentity, AppError> {
    let Some(token) = extract_admin_token(headers) else {
        return Err(AppError::unauthorized("missing admin token"));
    };

    db::admin_from_token(pool, &token)
        .await?
        .ok_or_else(|| AppError::unauthorized("invalid admin token"))
}

fn require_device_id(headers: &HeaderMap) -> Result<String, AppError> {
    extract_device_id(headers).ok_or_else(|| AppError::bad_request("missing device identity"))
}

fn viewer_to_summary(viewer: &ViewerIdentity) -> ViewerSummary {
    match viewer {
        ViewerIdentity::Device { id } => ViewerSummary::device(id.clone()),
        ViewerIdentity::User { id, username, .. } => ViewerSummary::user(*id, username.clone()),
    }
}

fn validate_credentials(username: &str, password: &str) -> Result<(), AppError> {
    if username.trim().len() < 3 {
        return Err(AppError::bad_request("username must be at least 3 characters"));
    }

    if password.len() < 8 {
        return Err(AppError::bad_request("password must be at least 8 characters"));
    }

    Ok(())
}
