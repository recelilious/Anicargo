use axum::{
    Json, Router,
    extract::{Path, Query, Request, State},
    http::HeaderMap,
    middleware,
    response::IntoResponse,
    routing::{get, post, put},
};
use futures::stream::{self, StreamExt};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;
use tower_http::{cors::CorsLayer, services::ServeFile, trace::TraceLayer};

use crate::{
    animegarden::AnimeGardenSearchProfile,
    auth::{
        AdminIdentity, ViewerIdentity, extract_admin_token, extract_device_id, extract_user_token,
    },
    bangumi::{BangumiClient, BangumiSearchQuery, EpisodeRaw, SearchFacets},
    config::AppConfig,
    db,
    discovery::ResourceDiscoveryCoordinator,
    downloads::{DownloadCoordinator, DownloadDemandInput},
    telemetry::{self, RuntimeMetrics},
    types::{
        ActivateDownloadResponse, AdminDashboardResponse, AdminDownloadCandidatesResponse,
        AdminDownloadExecutionEventsResponse, AdminDownloadExecutionsResponse,
        AdminDownloadQueueResponse, AdminRuntimeResponse, ApiEnvelope, AppError, AuthResponse,
        BootstrapResponse, CalendarDayDto, CalendarResponse, CredentialsRequest,
        EpisodePlaybackMediaDto, EpisodePlaybackResponse, FansubRuleDto, ForceDownloadResponse,
        HealthResponse, ResourceLibraryRequest, ResourceLibraryResponse, RuntimeHttpStatsDto,
        RuntimeOverviewDto, SearchRequest, SearchResponse, SubjectCardDto, SubjectDetailDto,
        SubjectDetailResponse, SubscriptionStateDto, ToggleSubscriptionResponse,
        UpdatePolicyRequest, UpsertFansubRuleRequest, ViewerSummary,
    },
    yuc::YucClient,
};

#[derive(Clone)]
pub struct AppState {
    pub config: AppConfig,
    pub pool: SqlitePool,
    pub bangumi: BangumiClient,
    pub yuc: YucClient,
    pub downloads: DownloadCoordinator,
    pub discovery: ResourceDiscoveryCoordinator,
    pub metrics: Arc<RuntimeMetrics>,
}

pub fn build_router(state: AppState) -> Router {
    let metrics = state.metrics.clone();

    Router::new()
        .route("/api/health", get(health))
        .route("/api/public/bootstrap", get(bootstrap))
        .route("/api/public/calendar", get(calendar))
        .route("/api/public/search", get(search))
        .route("/api/public/resources", get(resources))
        .route(
            "/api/public/subjects/{subject_id}/episodes/{episode_id}/playback",
            get(episode_playback),
        )
        .route("/api/public/subjects/{subject_id}", get(subject_detail))
        .route(
            "/api/public/media/{media_id}/stream",
            get(stream_media_file),
        )
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
        .route("/api/admin/runtime", get(admin_runtime))
        .route("/api/admin/downloads", get(admin_download_queue))
        .route(
            "/api/admin/downloads/{job_id}/execute",
            post(admin_activate_download),
        )
        .route(
            "/api/admin/downloads/{job_id}/candidates",
            get(admin_download_candidates),
        )
        .route(
            "/api/admin/downloads/{job_id}/executions",
            get(admin_download_executions),
        )
        .route(
            "/api/admin/executions/{execution_id}/events",
            get(admin_download_execution_events),
        )
        .route(
            "/api/admin/downloads/{subject_id}/force",
            post(force_download_job),
        )
        .route("/api/admin/policy", put(update_policy))
        .route("/api/admin/fansub-rules", post(create_fansub_rule))
        .with_state(state)
        .layer(middleware::from_fn_with_state(
            metrics,
            telemetry::track_http_metrics,
        ))
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
    let mut days = Vec::new();

    for day in state.bangumi.fetch_calendar().await? {
        let weekday = day.to_weekday();
        let cards = day
            .items
            .into_iter()
            .map(|item| item.to_calendar_card())
            .collect();
        let mut items = enrich_cards(&state.yuc, cards).await;
        sort_cards_by_broadcast_time(&mut items);

        days.push(CalendarDayDto { weekday, items });
    }

    Ok(Json(ApiEnvelope::new(CalendarResponse { days })))
}

async fn search(
    State(state): State<AppState>,
    Query(request): Query<SearchRequest>,
) -> Result<Json<ApiEnvelope<SearchResponse>>, AppError> {
    let page = request.page.unwrap_or(1).max(1);
    let page_size = request.page_size.unwrap_or(20).clamp(1, 60);
    let offset = (page - 1) * page_size;
    let query = BangumiSearchQuery {
        keyword: request.keyword.trim().to_owned(),
        sort: normalize_sort(request.sort.as_deref()),
        tags: normalize_terms(&request.tag),
        meta_tags: normalize_terms(&request.meta_tag),
        air_date_start: request.air_date_start.clone(),
        air_date_end: request.air_date_end.clone(),
        rating_min: request.rating_min,
        rating_max: request.rating_max,
        rating_count_min: request.rating_count_min,
        rating_count_max: request.rating_count_max,
        rank_min: request.rank_min,
        rank_max: request.rank_max,
        nsfw: normalize_nsfw_mode(request.nsfw_mode.as_deref()),
    };
    let response = state
        .bangumi
        .search_subjects(&query, page_size, offset)
        .await?;

    let mut years = response
        .data
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

    let mut tags = response
        .data
        .iter()
        .flat_map(|subject| subject.tags.iter().map(|tag| tag.name.clone()))
        .collect::<Vec<_>>();
    tags.sort_unstable();
    tags.dedup();

    let total = response.total.unwrap_or(response.data.len());
    let paged_items = enrich_cards(
        &state.yuc,
        response
            .data
            .into_iter()
            .map(|subject| subject.to_card())
            .collect(),
    )
    .await;

    Ok(Json(ApiEnvelope::new(SearchResponse {
        items: paged_items,
        facets: SearchFacets { years, tags },
        total,
        page,
        page_size,
        has_next_page: offset + page_size < total,
    })))
}

async fn resources(
    State(state): State<AppState>,
    Query(request): Query<ResourceLibraryRequest>,
) -> Result<Json<ApiEnvelope<ResourceLibraryResponse>>, AppError> {
    let page = request.page.unwrap_or(1).max(1);
    let page_size = request.page_size.unwrap_or(30).clamp(1, 60);
    let offset = (page - 1) * page_size;
    let (total, items) =
        db::list_resource_library_items(&state.pool, request.keyword.as_deref(), page_size, offset)
            .await?;

    Ok(Json(ApiEnvelope::new(ResourceLibraryResponse {
        items,
        total,
        page,
        page_size,
        has_next_page: offset + page_size < total,
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

    let (subject, episodes, episode_availability) = tokio::try_join!(
        state.bangumi.fetch_subject(subject_id),
        state.bangumi.fetch_episodes(subject_id),
        db::list_subject_episode_availability(&state.pool, subject_id)
    )?;

    let (is_subscribed, subscription_count) = if let Some(viewer) = viewer.as_ref() {
        db::subscription_state(&state.pool, viewer, subject_id).await?
    } else {
        (false, 0)
    };

    let subject = enrich_detail(&state.yuc, subject.to_detail()).await;

    Ok(Json(ApiEnvelope::new(SubjectDetailResponse {
        subject,
        episodes: episodes
            .into_iter()
            .map(|episode| {
                let (is_available, availability_note) =
                    resolve_episode_availability(&episode, &episode_availability);
                episode.to_dto(is_available, availability_note)
            })
            .collect(),
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

async fn episode_playback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((subject_id, episode_id)): Path<(i64, i64)>,
) -> Result<Json<ApiEnvelope<EpisodePlaybackResponse>>, AppError> {
    let device_id = extract_device_id(&headers);
    if let Some(id) = device_id.as_ref() {
        db::touch_device(&state.pool, id).await?;
    }

    let episode = state
        .bangumi
        .fetch_episodes(subject_id)
        .await?
        .into_iter()
        .find(|item| item.id == episode_id)
        .ok_or_else(|| AppError::not_found("episode not found on Bangumi"))?;

    let Some(episode_number) = episode.preferred_episode_number() else {
        return Ok(Json(ApiEnvelope::new(EpisodePlaybackResponse {
            bangumi_subject_id: subject_id,
            bangumi_episode_id: episode_id,
            episode_number: None,
            availability_state: "unmapped".to_owned(),
            note: "资源尚未建立剧集映射".to_owned(),
            media: None,
        })));
    };

    let media = db::find_episode_playback_media(&state.pool, subject_id, episode_number).await?;
    let response = if let Some(media) = media {
        EpisodePlaybackResponse {
            bangumi_subject_id: subject_id,
            bangumi_episode_id: episode_id,
            episode_number: Some(episode_number),
            availability_state: "ready".to_owned(),
            note: "可以直接播放".to_owned(),
            media: Some(EpisodePlaybackMediaDto {
                media_inventory_id: media.id,
                file_name: media.file_name,
                file_ext: media.file_ext,
                size_bytes: media.size_bytes,
                source_title: media.source_title,
                source_fansub_name: media.source_fansub_name,
                updated_at: media.updated_at,
                stream_url: format!("/api/public/media/{}/stream", media.id),
            }),
        }
    } else if db::has_partial_episode_media(&state.pool, subject_id, episode_number).await? {
        EpisodePlaybackResponse {
            bangumi_subject_id: subject_id,
            bangumi_episode_id: episode_id,
            episode_number: Some(episode_number),
            availability_state: "downloading".to_owned(),
            note: "资源下载中".to_owned(),
            media: None,
        }
    } else {
        EpisodePlaybackResponse {
            bangumi_subject_id: subject_id,
            bangumi_episode_id: episode_id,
            episode_number: Some(episode_number),
            availability_state: "missing".to_owned(),
            note: "资源尚未入库".to_owned(),
            media: None,
        }
    };

    Ok(Json(ApiEnvelope::new(response)))
}

async fn stream_media_file(
    State(state): State<AppState>,
    Path(media_id): Path<i64>,
    request: Request,
) -> Result<impl IntoResponse, AppError> {
    let media = db::resource_library_item_by_id(&state.pool, media_id)
        .await?
        .ok_or_else(|| AppError::not_found("media item not found"))?;

    let path = PathBuf::from(&media.absolute_path);
    if !path.exists() {
        return Err(AppError::not_found("media file not found on disk"));
    }

    ServeFile::new(path)
        .oneshot(request)
        .await
        .map_err(|_| AppError::internal("failed to stream media file"))
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
    let profile = resolve_subject_search_profile(&state.bangumi, subject_id).await;
    let download = state
        .downloads
        .reconcile_subscription_demand(
            &state.pool,
            DownloadDemandInput {
                bangumi_subject_id: subject_id,
                release_status: profile.release_status.clone(),
                subscription_count,
                threshold: policy.subscription_threshold,
                trigger_kind: "subscription",
                requested_by: viewer_to_download_requester(&viewer),
                force: false,
            },
        )
        .await?;

    if download.reason == "queued_threshold_job" {
        if let Some(job) = download.job.as_ref() {
            let discovery_profile = profile.to_discovery_profile();
            match state
                .discovery
                .discover_for_job(&state.pool, job, &discovery_profile, &policy)
                .await
            {
                Ok(_) => {
                    if let Err(error) = state
                        .downloads
                        .materialize_selected_candidate(
                            &state.pool,
                            &state.config.storage.media_root,
                            job.id,
                        )
                        .await
                    {
                        tracing::warn!(
                            job_id = job.id,
                            subject_id,
                            error = %error,
                            "Download execution activation failed after subscription-triggered queueing"
                        );
                    }
                }
                Err(error) => {
                    tracing::warn!(
                        job_id = job.id,
                        subject_id,
                        error = %error,
                        "Resource discovery failed after subscription-triggered queueing"
                    );
                }
            }
        }
    }

    Ok(Json(ApiEnvelope::new(ToggleSubscriptionResponse {
        bangumi_subject_id: subject_id,
        subscription: SubscriptionStateDto {
            is_subscribed,
            subscription_count,
            threshold: policy.subscription_threshold,
            source: viewer_to_summary(&viewer),
        },
        download,
    })))
}

async fn register(
    State(state): State<AppState>,
    Json(payload): Json<CredentialsRequest>,
) -> Result<Json<ApiEnvelope<AuthResponse>>, AppError> {
    validate_credentials(&payload.username, &payload.password)?;
    let (viewer, token) = db::register_user(
        &state.pool,
        &payload.username,
        &payload.password,
        &state.config.auth,
    )
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
    let (viewer, token) = db::login_user(
        &state.pool,
        &payload.username,
        &payload.password,
        &state.config.auth,
    )
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
    let (admin, token) = db::login_admin(
        &state.pool,
        &payload.username,
        &payload.password,
        &state.config.auth,
    )
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

async fn admin_download_queue(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiEnvelope<AdminDownloadQueueResponse>>, AppError> {
    require_admin(&state.pool, &headers).await?;
    let items = state.downloads.list_jobs(&state.pool, 50).await?;

    Ok(Json(ApiEnvelope::new(AdminDownloadQueueResponse { items })))
}

async fn admin_runtime(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<ApiEnvelope<AdminRuntimeResponse>>, AppError> {
    require_admin(&state.pool, &headers).await?;

    let snapshot = state.metrics.snapshot();
    let overview = db::runtime_overview(&state.pool).await?;

    Ok(Json(ApiEnvelope::new(AdminRuntimeResponse {
        server_address: snapshot.server_address,
        uptime_seconds: snapshot.uptime.as_secs(),
        uptime_label: format_runtime_duration(snapshot.uptime),
        log_dir: state.config.telemetry.log_dir.display().to_string(),
        download_engine: state.downloads.engine_name().to_owned(),
        http: RuntimeHttpStatsDto {
            active_requests: snapshot.active_requests,
            total_requests: snapshot.request_total,
            failed_requests: snapshot.request_failures,
            incoming_bytes: snapshot.request_bytes,
            outgoing_bytes: snapshot.response_bytes,
            last_route: snapshot.last_route,
            last_status: snapshot.last_status,
            last_latency_ms: snapshot.last_latency_ms,
        },
        runtime: RuntimeOverviewDto {
            devices: overview.devices,
            users: overview.users,
            active_sessions: overview.active_sessions,
            subscriptions: overview.subscriptions,
            open_download_jobs: overview.open_download_jobs,
            jobs_with_selection: overview.jobs_with_selection,
            running_searches: overview.running_searches,
            resource_candidates: overview.resource_candidates,
            active_executions: overview.active_executions,
            downloaded_bytes: overview.downloaded_bytes,
            uploaded_bytes: overview.uploaded_bytes,
            download_rate_bytes: overview.download_rate_bytes,
            upload_rate_bytes: overview.upload_rate_bytes,
            peer_count: overview.peer_count,
        },
    })))
}

async fn admin_download_candidates(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(job_id): Path<i64>,
) -> Result<Json<ApiEnvelope<AdminDownloadCandidatesResponse>>, AppError> {
    require_admin(&state.pool, &headers).await?;
    let items = db::list_resource_candidates(&state.pool, job_id).await?;

    Ok(Json(ApiEnvelope::new(AdminDownloadCandidatesResponse {
        download_job_id: job_id,
        items,
    })))
}

async fn admin_activate_download(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(job_id): Path<i64>,
) -> Result<Json<ApiEnvelope<ActivateDownloadResponse>>, AppError> {
    require_admin(&state.pool, &headers).await?;
    let decision = state
        .downloads
        .materialize_selected_candidate(&state.pool, &state.config.storage.media_root, job_id)
        .await?;

    Ok(Json(ApiEnvelope::new(ActivateDownloadResponse {
        download_job_id: job_id,
        decision,
    })))
}

async fn admin_download_executions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(job_id): Path<i64>,
) -> Result<Json<ApiEnvelope<AdminDownloadExecutionsResponse>>, AppError> {
    require_admin(&state.pool, &headers).await?;
    let items = state.downloads.list_executions(&state.pool, job_id).await?;

    Ok(Json(ApiEnvelope::new(AdminDownloadExecutionsResponse {
        download_job_id: job_id,
        items,
    })))
}

async fn admin_download_execution_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(execution_id): Path<i64>,
) -> Result<Json<ApiEnvelope<AdminDownloadExecutionEventsResponse>>, AppError> {
    require_admin(&state.pool, &headers).await?;
    let items = db::list_download_execution_events(&state.pool, execution_id).await?;

    Ok(Json(ApiEnvelope::new(
        AdminDownloadExecutionEventsResponse {
            download_execution_id: execution_id,
            items,
        },
    )))
}

async fn force_download_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(subject_id): Path<i64>,
) -> Result<Json<ApiEnvelope<ForceDownloadResponse>>, AppError> {
    let admin = require_admin(&state.pool, &headers).await?;
    let policy = db::load_policy(&state.pool).await?;
    let subscription_count = db::total_subscription_count(&state.pool, subject_id).await?;
    let profile = resolve_subject_search_profile(&state.bangumi, subject_id).await;
    let decision = state
        .downloads
        .reconcile_subscription_demand(
            &state.pool,
            DownloadDemandInput {
                bangumi_subject_id: subject_id,
                release_status: profile.release_status.clone(),
                subscription_count,
                threshold: policy.subscription_threshold,
                trigger_kind: "admin_force",
                requested_by: format!("admin:{}", admin.username),
                force: true,
            },
        )
        .await?;

    if let Some(job) = decision.job.as_ref() {
        let discovery_profile = profile.to_discovery_profile();
        match state
            .discovery
            .discover_for_job(&state.pool, job, &discovery_profile, &policy)
            .await
        {
            Ok(_) => {
                if let Err(error) = state
                    .downloads
                    .materialize_selected_candidate(
                        &state.pool,
                        &state.config.storage.media_root,
                        job.id,
                    )
                    .await
                {
                    tracing::warn!(
                        job_id = job.id,
                        subject_id,
                        error = %error,
                        "Download execution activation failed after admin force queueing"
                    );
                }
            }
            Err(error) => {
                tracing::warn!(
                    job_id = job.id,
                    subject_id,
                    error = %error,
                    "Resource discovery failed after admin force queueing"
                );
            }
        }
    }

    Ok(Json(ApiEnvelope::new(ForceDownloadResponse {
        bangumi_subject_id: subject_id,
        decision,
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

    Ok(fallback_device_id.map(|id| ViewerIdentity::Device { id: id.to_owned() }))
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

fn viewer_to_download_requester(viewer: &ViewerIdentity) -> String {
    match viewer {
        ViewerIdentity::Device { id } => format!("device:{id}"),
        ViewerIdentity::User { id, username } => format!("user:{id}:{username}"),
    }
}

fn validate_credentials(username: &str, password: &str) -> Result<(), AppError> {
    if username.trim().len() < 3 {
        return Err(AppError::bad_request(
            "username must be at least 3 characters",
        ));
    }

    if password.len() < 8 {
        return Err(AppError::bad_request(
            "password must be at least 8 characters",
        ));
    }

    Ok(())
}

async fn enrich_cards(yuc: &YucClient, cards: Vec<SubjectCardDto>) -> Vec<SubjectCardDto> {
    stream::iter(cards.into_iter().map(|card| {
        let yuc = yuc.clone();
        async move { yuc.enrich_card(card).await }
    }))
    .buffered(8)
    .collect()
    .await
}

async fn enrich_detail(yuc: &YucClient, detail: SubjectDetailDto) -> SubjectDetailDto {
    yuc.enrich_detail(detail).await
}

async fn resolve_subject_search_profile(
    bangumi: &BangumiClient,
    subject_id: i64,
) -> AnimeGardenSearchProfileWithStatus {
    match bangumi.fetch_subject(subject_id).await {
        Ok(subject) => AnimeGardenSearchProfileWithStatus {
            bangumi_subject_id: subject_id,
            title: subject.name.clone(),
            title_cn: subject.name_cn.clone(),
            release_status: subject.to_card().release_status,
        },
        Err(error) => {
            tracing::warn!(
                subject_id,
                error = %error,
                "Failed to resolve subject metadata for resource discovery; falling back to subject id only"
            );
            AnimeGardenSearchProfileWithStatus {
                bangumi_subject_id: subject_id,
                title: String::new(),
                title_cn: String::new(),
                release_status: "completed".to_owned(),
            }
        }
    }
}

#[derive(Debug, Clone)]
struct AnimeGardenSearchProfileWithStatus {
    bangumi_subject_id: i64,
    title: String,
    title_cn: String,
    release_status: String,
}

impl AnimeGardenSearchProfileWithStatus {
    fn to_discovery_profile(&self) -> AnimeGardenSearchProfile {
        AnimeGardenSearchProfile {
            bangumi_subject_id: self.bangumi_subject_id,
            title: self.title.clone(),
            title_cn: self.title_cn.clone(),
        }
    }
}

fn normalize_terms(values: &[String]) -> Vec<String> {
    values
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .collect()
}

fn normalize_sort(sort: Option<&str>) -> String {
    match sort.unwrap_or("score") {
        "match" | "heat" | "rank" | "score" => sort.unwrap_or("score").to_owned(),
        _ => "score".to_owned(),
    }
}

fn normalize_nsfw_mode(mode: Option<&str>) -> Option<bool> {
    match mode.unwrap_or("any") {
        "only" => Some(true),
        "safe" => Some(false),
        _ => None,
    }
}

fn sort_cards_by_broadcast_time(items: &mut [SubjectCardDto]) {
    items.sort_by(|left, right| {
        match (
            parse_broadcast_time(left.broadcast_time.as_deref()),
            parse_broadcast_time(right.broadcast_time.as_deref()),
        ) {
            (Some(left_key), Some(right_key)) => left_key.cmp(&right_key),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left
                .title_cn
                .cmp(&right.title_cn)
                .then_with(|| left.title.cmp(&right.title)),
        }
    });
}

fn parse_broadcast_time(value: Option<&str>) -> Option<u16> {
    let value = value?.trim();
    let (hour, minute) = value.split_once(':')?;
    let hour = hour.parse::<u16>().ok()?;
    let minute = minute.parse::<u16>().ok()?;
    Some(hour * 60 + minute)
}

fn resolve_episode_availability(
    episode: &EpisodeRaw,
    availability: &[db::SubjectEpisodeAvailability],
) -> (bool, Option<String>) {
    let Some(episode_number) = episode.preferred_episode_number() else {
        return (false, Some("资源尚未建立剧集映射".to_owned()));
    };

    if availability
        .iter()
        .any(|item| item.status == "ready" && availability_covers_episode(item, episode_number))
    {
        return (true, Some("已入库".to_owned()));
    }

    if availability
        .iter()
        .any(|item| item.status == "partial" && availability_covers_episode(item, episode_number))
    {
        return (false, Some("资源下载中".to_owned()));
    }

    (false, Some("资源尚未入库".to_owned()))
}

fn availability_covers_episode(item: &db::SubjectEpisodeAvailability, episode_number: f64) -> bool {
    let Some(start) = item.episode_index else {
        return false;
    };
    let end = item.episode_end_index.unwrap_or(start);
    let epsilon = if item.is_collection {
        0.001
    } else {
        f64::EPSILON
    };

    episode_number + epsilon >= start && episode_number - epsilon <= end
}

fn format_runtime_duration(duration: std::time::Duration) -> String {
    let total = duration.as_secs();
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}
