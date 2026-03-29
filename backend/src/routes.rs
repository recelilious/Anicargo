use axum::{
    Json, Router,
    extract::{Path, Query, Request, State},
    http::HeaderMap,
    middleware,
    response::IntoResponse,
    routing::{get, post, put},
};
use chrono::{FixedOffset, NaiveDate, Utc};
use chrono_tz::Tz;
use futures::stream::{self, StreamExt};
use sqlx::SqlitePool;
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
};
use tokio::time::{Duration as TokioDuration, sleep, timeout};
use tower::ServiceExt;
use tower_http::{cors::CorsLayer, services::ServeFile, trace::TraceLayer};

use crate::{
    animegarden::AnimeGardenSearchProfile,
    auth::{
        AdminIdentity, ViewerIdentity, extract_admin_token, extract_device_id, extract_user_token,
    },
    bangumi::{BangumiClient, BangumiSearchQuery, EpisodeRaw, SearchFacets},
    catalog_cache,
    config::AppConfig,
    db,
    discovery::{
        ResourceDiscoveryCoordinator, candidate_priority_key, infer_season_hint_from_texts,
        within_replacement_window,
    },
    downloads::{DownloadCoordinator, DownloadDemandInput, DownloadRuntimeSettings},
    media, season_catalog,
    telemetry::{self, RuntimeMetrics},
    types::{
        ActivateDownloadResponse, ActiveDownloadDto, ActiveDownloadsResponse,
        AdminDashboardResponse, AdminDownloadCandidatesResponse,
        AdminDownloadExecutionEventsResponse, AdminDownloadExecutionsResponse,
        AdminDownloadQueueResponse, AdminRuntimeResponse, ApiEnvelope, AppError, AuthResponse,
        BootstrapResponse, CalendarResponse, CatalogManifestResponse, CatalogPageResponse,
        CredentialsRequest, DownloadJobDto, EpisodePlaybackMediaDto, EpisodePlaybackResponse,
        FansubRuleDto, ForceDownloadResponse, HealthResponse, PlaybackHistoryItemDto,
        PlaybackHistoryRecordRequest, PlaybackHistoryResponse, PolicyDto, ResourceCandidateDto,
        ResourceLibraryRequest, ResourceLibraryResponse, RuntimeHttpStatsDto, RuntimeOverviewDto,
        ScheduleDisplayQuery, SearchRequest, SearchResponse, SubjectCardDto,
        SubjectCollectionRequest, SubjectCollectionResponse, SubjectDetailDto,
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
        .route("/api/public/catalogs/manifest", get(catalog_manifest))
        .route("/api/public/catalogs/{kind}", get(catalog_page))
        .route("/api/public/search", get(search))
        .route("/api/public/subscriptions", get(subscriptions))
        .route("/api/public/history", get(playback_history))
        .route("/api/public/resources", get(resources))
        .route("/api/public/downloads/active", get(active_downloads))
        .route(
            "/api/public/subjects/{subject_id}/download-status",
            get(subject_download_status),
        )
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
        .route(
            "/api/public/history/playback",
            post(record_playback_history),
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
    Query(display_query): Query<ScheduleDisplayQuery>,
) -> Result<Json<ApiEnvelope<CalendarResponse>>, AppError> {
    let display = schedule_display_options(&display_query);
    let days = season_catalog::load_current_season_calendar(
        &state.yuc,
        &state.pool,
        &state.bangumi,
        &display,
    )
    .await?;

    Ok(Json(ApiEnvelope::new(CalendarResponse { days })))
}

async fn catalog_manifest(
    State(state): State<AppState>,
) -> Result<Json<ApiEnvelope<CatalogManifestResponse>>, AppError> {
    let manifest =
        catalog_cache::load_catalog_manifest(&state.yuc, &state.pool, &state.bangumi).await?;
    Ok(Json(ApiEnvelope::new(manifest)))
}

async fn catalog_page(
    State(state): State<AppState>,
    Path(kind): Path<String>,
) -> Result<Json<ApiEnvelope<CatalogPageResponse>>, AppError> {
    let page =
        catalog_cache::load_catalog_page(&state.yuc, &state.pool, &state.bangumi, &kind).await?;
    Ok(Json(ApiEnvelope::new(page)))
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

async fn subscriptions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(request): Query<SubjectCollectionRequest>,
) -> Result<Json<ApiEnvelope<SubjectCollectionResponse>>, AppError> {
    let device_id = require_device_id(&headers)?;
    db::touch_device(&state.pool, &device_id).await?;
    let viewer = resolve_viewer(&state.pool, &headers, &device_id).await?;

    let page = request.page.unwrap_or(1).max(1);
    let page_size = request.page_size.unwrap_or(30).clamp(1, 60);
    let keyword = request.keyword.unwrap_or_default();
    let sort = normalize_collection_sort(request.sort.as_deref());
    let subscriptions = db::list_viewer_subscription_subjects(&state.pool, &viewer).await?;
    let mut items =
        hydrate_subscription_cards(&state.bangumi, &state.yuc, subscriptions, &keyword).await;

    sort_subscription_items(&mut items, &sort);

    let total = items.len();
    let offset = (page - 1) * page_size;
    let paged_items = items
        .into_iter()
        .skip(offset)
        .take(page_size)
        .map(|item| item.card)
        .collect();

    Ok(Json(ApiEnvelope::new(SubjectCollectionResponse {
        items: paged_items,
        total,
        page,
        page_size,
        has_next_page: offset + page_size < total,
    })))
}

async fn playback_history(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(request): Query<ResourceLibraryRequest>,
) -> Result<Json<ApiEnvelope<PlaybackHistoryResponse>>, AppError> {
    let device_id = require_device_id(&headers)?;
    db::touch_device(&state.pool, &device_id).await?;
    let viewer = resolve_viewer(&state.pool, &headers, &device_id).await?;
    let page = request.page.unwrap_or(1).max(1);
    let page_size = request.page_size.unwrap_or(30).clamp(1, 60);
    let offset = (page - 1) * page_size;
    let (total, history) =
        db::list_viewer_playback_history(&state.pool, &viewer, page_size, offset).await?;
    let items = hydrate_playback_history(&state.bangumi, &state.yuc, history).await;

    Ok(Json(ApiEnvelope::new(PlaybackHistoryResponse {
        items,
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
    let (total, total_size_bytes, items) =
        db::list_resource_library_items(&state.pool, request.keyword.as_deref(), page_size, offset)
            .await?;

    Ok(Json(ApiEnvelope::new(ResourceLibraryResponse {
        items,
        total,
        total_size_bytes,
        page,
        page_size,
        has_next_page: offset + page_size < total,
    })))
}

async fn active_downloads(
    State(state): State<AppState>,
) -> Result<Json<ApiEnvelope<ActiveDownloadsResponse>>, AppError> {
    match timeout(
        TokioDuration::from_secs(2),
        state
            .downloads
            .sync_active_executions(&state.pool, &state.config.storage.media_root),
    )
    .await
    {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            tracing::warn!(error = %error, "Active downloads request failed to refresh execution state")
        }
        Err(_) => {
            tracing::warn!("Active downloads request timed out while refreshing execution state")
        }
    }

    let executions =
        db::list_active_download_executions(&state.pool, state.downloads.engine_name(), 24).await?;
    let items = hydrate_active_downloads(&state.bangumi, &state.yuc, executions).await;

    Ok(Json(ApiEnvelope::new(ActiveDownloadsResponse { items })))
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

    let (subject, episodes, episode_availability, download_status) = tokio::try_join!(
        state.bangumi.fetch_subject(subject_id),
        state.bangumi.fetch_episodes(subject_id),
        db::list_subject_episode_availability(&state.pool, subject_id),
        db::subject_download_status(&state.pool, subject_id)
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
        download_status: download_status,
    })))
}

async fn subject_download_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(subject_id): Path<i64>,
) -> Result<Json<ApiEnvelope<Option<crate::types::SubjectDownloadStatusDto>>>, AppError> {
    let device_id = extract_device_id(&headers);
    if let Some(id) = device_id.as_ref() {
        db::touch_device(&state.pool, id).await?;
    }

    let status = db::subject_download_status(&state.pool, subject_id).await?;
    Ok(Json(ApiEnvelope::new(status)))
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
    let profile = resolve_subject_search_profile(&state.pool, &state.bangumi, subject_id).await;
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

    if matches!(
        download.reason.as_str(),
        "queued_threshold_job" | "reused_existing_threshold_job"
    ) {
        if let Some(job) = download.job.as_ref() {
            let background_state = state.clone();
            let background_job = job.clone();
            let background_policy = policy.clone();
            let discovery_profile = profile.to_discovery_profile();
            tokio::spawn(async move {
                run_download_pipeline(
                    background_state,
                    background_job,
                    discovery_profile,
                    background_policy,
                    "subscription trigger",
                )
                .await;
            });
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

async fn record_playback_history(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PlaybackHistoryRecordRequest>,
) -> Result<Json<ApiEnvelope<bool>>, AppError> {
    let device_id = require_device_id(&headers)?;
    db::touch_device(&state.pool, &device_id).await?;
    let viewer = resolve_viewer(&state.pool, &headers, &device_id).await?;

    db::record_playback_history(
        &state.pool,
        &viewer,
        payload.bangumi_subject_id,
        payload.bangumi_episode_id,
        payload.media_inventory_id,
    )
    .await?;

    Ok(Json(ApiEnvelope::new(true)))
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
    let profile = resolve_subject_search_profile(&state.pool, &state.bangumi, subject_id).await;
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
        run_download_pipeline(
            state.clone(),
            job.clone(),
            discovery_profile,
            policy.clone(),
            "admin force trigger",
        )
        .await;
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
        payload.max_concurrent_downloads,
        payload.upload_limit_mb,
        payload.download_limit_mb,
    )
    .await?;

    state
        .downloads
        .apply_runtime_settings(DownloadRuntimeSettings::new(
            policy.max_concurrent_downloads as usize,
            policy.upload_limit_mb.max(0) as u64,
            policy.download_limit_mb.max(0) as u64,
        ))
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

const DISCOVERY_MAX_ATTEMPTS: usize = 3;
const DISCOVERY_RETRY_BASE_DELAY_MS: u64 = 3_000;

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

fn schedule_display_options(
    query: &ScheduleDisplayQuery,
) -> season_catalog::ScheduleDisplayOptions {
    season_catalog::ScheduleDisplayOptions {
        timezone: query
            .timezone
            .as_deref()
            .and_then(parse_client_timezone)
            .unwrap_or(Tz::UTC),
        deep_night_mode: query.deep_night_mode.unwrap_or(true),
    }
}

fn parse_client_timezone(value: &str) -> Option<Tz> {
    value.trim().parse::<Tz>().ok()
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
    pool: &SqlitePool,
    bangumi: &BangumiClient,
    subject_id: i64,
) -> AnimeGardenSearchProfileWithStatus {
    let cached = match db::cached_bangumi_subject_summary(pool, subject_id).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(
                subject_id,
                error = %error,
                "Failed to read cached Bangumi subject summary for resource discovery"
            );
            None
        }
    };

    match bangumi.fetch_subject(subject_id).await {
        Ok(subject) => {
            let episodes = match bangumi.fetch_episodes(subject_id).await {
                Ok(episodes) => episodes,
                Err(error) => {
                    tracing::warn!(
                        subject_id,
                        error = %error,
                        "Failed to fetch Bangumi episodes while resolving subject status for discovery"
                    );
                    Vec::new()
                }
            };

            let release_status =
                season_catalog::derive_release_status(&subject, &episodes).to_owned();
            tracing::info!(
                subject_id,
                release_status = %release_status,
                cached_release_status = ?cached.as_ref().map(|item| item.release_status.clone()),
                episode_count = episodes.len(),
                "Resolved live Bangumi subject profile for resource discovery"
            );

            AnimeGardenSearchProfileWithStatus {
                bangumi_subject_id: subject_id,
                title: subject.name.clone(),
                title_cn: subject.name_cn.clone(),
                release_status,
                season_hint: infer_season_hint_from_texts([
                    subject.name.as_str(),
                    subject.name_cn.as_str(),
                ]),
            }
        }
        Err(error) => {
            tracing::warn!(
                subject_id,
                error = %error,
                cached_release_status = ?cached.as_ref().map(|item| item.release_status.clone()),
                "Failed to resolve live Bangumi subject metadata for resource discovery; falling back to cache"
            );

            if let Some(cached) = cached {
                let season_hint =
                    infer_season_hint_from_texts([cached.title.as_str(), cached.title_cn.as_str()]);
                tracing::info!(
                    subject_id,
                    release_status = %cached.release_status,
                    "Using cached Bangumi subject profile for resource discovery fallback"
                );
                return AnimeGardenSearchProfileWithStatus {
                    bangumi_subject_id: subject_id,
                    title: cached.title,
                    title_cn: cached.title_cn,
                    release_status: cached.release_status,
                    season_hint,
                };
            }

            AnimeGardenSearchProfileWithStatus {
                bangumi_subject_id: subject_id,
                title: String::new(),
                title_cn: String::new(),
                release_status: "completed".to_owned(),
                season_hint: None,
            }
        }
    }
}

async fn resolve_download_episode_targets(
    state: &AppState,
    job: &crate::types::DownloadJobDto,
    policy: &crate::types::PolicyDto,
) -> Result<AiringEpisodeTargets, AppError> {
    let (episodes, availability, executions, current_selected) = tokio::try_join!(
        state.bangumi.fetch_episodes(job.bangumi_subject_id),
        db::list_subject_episode_availability(&state.pool, job.bangumi_subject_id),
        db::list_download_executions(&state.pool, job.id),
        db::current_selected_candidate_for_job(&state.pool, job.id)
    )?;

    let mut tracked_episodes = episodes
        .into_iter()
        .filter_map(|episode| {
            let episode_number = episode.preferred_episode_number()?;
            match job.release_status.as_str() {
                "upcoming" => None,
                "airing" => episode_has_aired(&episode).then_some(episode_number),
                _ => Some(episode_number),
            }
        })
        .collect::<Vec<_>>();
    tracked_episodes.sort_by(|left, right| left.total_cmp(right));
    tracked_episodes.dedup_by(|left, right| (*left - *right).abs() < 0.001);

    let latest = tracked_episodes.last().copied();
    let backlog = tracked_episodes
        .iter()
        .copied()
        .filter(|episode_number| Some(*episode_number) != latest)
        .filter(|episode_number| {
            !episode_already_tracked(*episode_number, &availability, &executions)
        })
        .collect::<Vec<_>>();
    let latest_should_search = latest.is_some_and(|latest_episode| {
        if !episode_already_tracked(latest_episode, &availability, &executions) {
            return true;
        }

        job.release_status == "airing"
            && current_selected.as_ref().is_some_and(|candidate| {
                candidate_covers_episode(candidate, latest_episode)
                    && within_replacement_window(
                        job.selection_updated_at.as_deref(),
                        policy.replacement_window_hours,
                    )
            })
    });

    let targets = AiringEpisodeTargets {
        backlog,
        latest: latest.filter(|_| latest_should_search),
    };

    tracing::info!(
        job_id = job.id,
        subject_id = job.bangumi_subject_id,
        release_status = %job.release_status,
        backlog = ?targets.backlog,
        latest = ?targets.latest,
        "Resolved Bangumi episode targets for sequential download planning"
    );

    Ok(targets)
}

async fn run_download_pipeline(
    state: AppState,
    job: crate::types::DownloadJobDto,
    discovery_profile: AnimeGardenSearchProfile,
    policy: crate::types::PolicyDto,
    reason: &'static str,
) {
    if let Err(error) = db::update_download_job_lifecycle(
        &state.pool,
        job.id,
        "searching",
        Some("Preparing resource discovery"),
    )
    .await
    {
        tracing::warn!(
            job_id = job.id,
            subject_id = job.bangumi_subject_id,
            error = %error,
            "Failed to mark download job as searching before resource discovery"
        );
    }
    if let Err(error) = db::update_download_job_search_status(
        &state.pool,
        job.id,
        "preparing",
        Some("Preparing resource discovery"),
    )
    .await
    {
        tracing::warn!(
            job_id = job.id,
            subject_id = job.bangumi_subject_id,
            error = %error,
            "Failed to mark download job search status as preparing"
        );
    }
    tracing::info!(
        job_id = job.id,
        subject_id = job.bangumi_subject_id,
        release_status = %job.release_status,
        reason,
        "Starting download pipeline"
    );
    if job.release_status == "upcoming" {
        if let Err(error) = db::update_download_job_lifecycle(
            &state.pool,
            job.id,
            "queued",
            Some("Waiting for the first broadcast before starting resource discovery"),
        )
        .await
        {
            tracing::warn!(
                job_id = job.id,
                subject_id = job.bangumi_subject_id,
                error = %error,
                "Failed to persist waiting state for upcoming subject"
            );
        }
        if let Err(error) = db::update_download_job_search_status(
            &state.pool,
            job.id,
            "idle",
            Some("Waiting for the first broadcast before starting resource discovery"),
        )
        .await
        {
            tracing::warn!(
                job_id = job.id,
                subject_id = job.bangumi_subject_id,
                error = %error,
                "Failed to persist idle search state for upcoming subject"
            );
        }
        tracing::info!(
            job_id = job.id,
            subject_id = job.bangumi_subject_id,
            reason,
            "Skipping download pipeline because the subject is still upcoming"
        );
        return;
    }

    let episode_targets = match resolve_download_episode_targets(&state, &job, &policy).await {
        Ok(targets) => Some(targets),
        Err(error) => {
            tracing::warn!(
                job_id = job.id,
                subject_id = job.bangumi_subject_id,
                error = %error,
                reason,
                "Failed to resolve Bangumi episode targets; falling back to broad discovery"
            );
            None
        }
    };
    let search_targets = episode_targets
        .as_ref()
        .map(AiringEpisodeTargets::search_targets);

    match discover_candidates_with_retries(
        &state,
        &job,
        &discovery_profile,
        &policy,
        search_targets.as_deref(),
        reason,
    )
    .await
    {
        Ok(candidates) => {
            tracing::info!(
                job_id = job.id,
                subject_id = job.bangumi_subject_id,
                candidate_count = candidates.len(),
                reason,
                "Resource discovery finished"
            );
            let activation_result =
                apply_download_plan(&state, &job, &policy, &candidates, episode_targets.as_ref())
                    .await;

            if let Err(error) = activation_result {
                let error_message = format!("Download activation failed: {error}");
                tracing::warn!(
                    job_id = job.id,
                    subject_id = job.bangumi_subject_id,
                    error = %error,
                    reason,
                    "Download execution activation failed after background queueing"
                );
                if let Err(update_error) = db::update_download_job_lifecycle(
                    &state.pool,
                    job.id,
                    "queued",
                    Some(&error_message),
                )
                .await
                {
                    tracing::warn!(
                        job_id = job.id,
                        subject_id = job.bangumi_subject_id,
                        error = %update_error,
                        "Failed to persist download activation failure notes"
                    );
                }
            } else {
                tracing::info!(
                    job_id = job.id,
                    subject_id = job.bangumi_subject_id,
                    reason,
                    "Download pipeline finished"
                );
            }
        }
        Err(error) => {
            let error_message = format!("Resource discovery failed: {error}");
            tracing::warn!(
                job_id = job.id,
                subject_id = job.bangumi_subject_id,
                error = %error,
                reason,
                "Resource discovery failed after background queueing"
            );
            if let Err(update_error) = db::update_download_job_search_status(
                &state.pool,
                job.id,
                "failed",
                Some(&error_message),
            )
            .await
            {
                tracing::warn!(
                    job_id = job.id,
                    subject_id = job.bangumi_subject_id,
                    error = %update_error,
                    "Failed to persist resource discovery failure state"
                );
            }
            if let Err(update_error) = db::update_download_job_lifecycle(
                &state.pool,
                job.id,
                "queued",
                Some(&error_message),
            )
            .await
            {
                tracing::warn!(
                    job_id = job.id,
                    subject_id = job.bangumi_subject_id,
                    error = %update_error,
                    "Failed to persist queued lifecycle after resource discovery failure"
                );
            }
        }
    }
}

#[derive(Debug, Default)]
struct DownloadPlan {
    candidate_chains: Vec<Vec<i64>>,
}

async fn discover_candidates_with_retries(
    state: &AppState,
    job: &DownloadJobDto,
    discovery_profile: &AnimeGardenSearchProfile,
    policy: &PolicyDto,
    search_targets: Option<&[f64]>,
    reason: &'static str,
) -> Result<Vec<ResourceCandidateDto>, AppError> {
    for attempt in 1..=DISCOVERY_MAX_ATTEMPTS {
        match state
            .discovery
            .discover_for_job(&state.pool, job, discovery_profile, policy, search_targets)
            .await
        {
            Ok(candidates) => return Ok(candidates),
            Err(error @ AppError::Upstream(_)) if attempt < DISCOVERY_MAX_ATTEMPTS => {
                let delay_ms = DISCOVERY_RETRY_BASE_DELAY_MS * attempt as u64;
                let note = format!(
                    "AnimeGarden discovery attempt {attempt}/{DISCOVERY_MAX_ATTEMPTS} failed; retrying in {}s",
                    delay_ms / 1000
                );
                tracing::warn!(
                    job_id = job.id,
                    subject_id = job.bangumi_subject_id,
                    attempt,
                    max_attempts = DISCOVERY_MAX_ATTEMPTS,
                    reason,
                    error = %error,
                    delay_ms,
                    "AnimeGarden discovery attempt failed; scheduling retry"
                );
                if let Err(update_error) = db::update_download_job_search_status(
                    &state.pool,
                    job.id,
                    "retrying",
                    Some(&note),
                )
                .await
                {
                    tracing::warn!(
                        job_id = job.id,
                        subject_id = job.bangumi_subject_id,
                        error = %update_error,
                        "Failed to persist resource discovery retry state"
                    );
                }
                sleep(TokioDuration::from_millis(delay_ms)).await;
            }
            Err(error) => return Err(error),
        }
    }

    Err(AppError::upstream(
        "AnimeGarden discovery retries exhausted",
    ))
}

#[derive(Debug, Clone, Default)]
struct AiringEpisodeTargets {
    backlog: Vec<f64>,
    latest: Option<f64>,
}

impl AiringEpisodeTargets {
    fn search_targets(&self) -> Vec<f64> {
        let mut targets = self.backlog.clone();
        if let Some(latest) = self.latest {
            if !targets.iter().any(|value| (*value - latest).abs() < 0.001) {
                targets.push(latest);
            }
        }
        targets.sort_by(|left, right| left.total_cmp(right));
        targets
    }
}

async fn apply_download_plan(
    state: &AppState,
    job: &crate::types::DownloadJobDto,
    policy: &crate::types::PolicyDto,
    candidates: &[ResourceCandidateDto],
    targets: Option<&AiringEpisodeTargets>,
) -> Result<(), AppError> {
    let plan = if job.release_status == "airing" {
        build_airing_download_plan(&state.pool, job, policy, candidates, targets).await?
    } else {
        build_completed_download_plan(&state.pool, job, policy, candidates, targets).await?
    };

    tracing::info!(
        job_id = job.id,
        subject_id = job.bangumi_subject_id,
        candidate_chains = ?plan.candidate_chains,
        "Built download plan"
    );

    for (index, candidate_chain) in plan.candidate_chains.into_iter().enumerate() {
        materialize_candidate_chain_with_peer_fallback(state, job, index as u32, &candidate_chain)
            .await?;
    }

    Ok(())
}

async fn materialize_candidate_chain_with_peer_fallback(
    state: &AppState,
    job: &crate::types::DownloadJobDto,
    priority: u32,
    candidate_chain: &[i64],
) -> Result<(), AppError> {
    let mut last_error = None;

    for candidate_id in candidate_chain {
        let probed_peer_count = match state
            .downloads
            .probe_candidate_with_priority(
                &state.pool,
                &state.config.storage.media_root,
                job.id,
                *candidate_id,
                priority,
            )
            .await
        {
            Ok(probe) if probe.peer_count == Some(0) => {
                tracing::info!(
                    job_id = job.id,
                    subject_id = job.bangumi_subject_id,
                    candidate_id,
                    priority,
                    notes = ?probe.notes,
                    "Skipping candidate because source inspection found zero peers"
                );
                continue;
            }
            Ok(probe) => {
                tracing::info!(
                    job_id = job.id,
                    subject_id = job.bangumi_subject_id,
                    candidate_id,
                    priority,
                    peer_count = ?probe.peer_count,
                    notes = ?probe.notes,
                    "Candidate source inspection passed"
                );
                probe.peer_count
            }
            Err(error) => {
                tracing::warn!(
                    job_id = job.id,
                    subject_id = job.bangumi_subject_id,
                    candidate_id,
                    priority,
                    error = %error,
                    "Candidate source inspection failed; trying next scored candidate"
                );
                last_error = Some(error);
                continue;
            }
        };

        match state
            .downloads
            .materialize_candidate_with_priority(
                &state.pool,
                &state.config.storage.media_root,
                job.id,
                *candidate_id,
                priority,
            )
            .await
        {
            Ok(decision) => {
                if let (Some(execution), Some(probe_peer_count)) =
                    (decision.execution.as_ref(), probed_peer_count)
                {
                    if execution.peer_count == 0 && probe_peer_count > 0 {
                        db::update_download_execution_metrics(
                            &state.pool,
                            execution.id,
                            &execution.state,
                            execution.downloaded_bytes,
                            execution.source_size_bytes.max(execution.downloaded_bytes),
                            execution.uploaded_bytes,
                            execution.download_rate_bytes,
                            execution.upload_rate_bytes,
                            probe_peer_count,
                            Some("Initialized queued execution with preflight peer count"),
                        )
                        .await?;
                    }
                }
                return Ok(());
            }
            Err(error) => {
                tracing::warn!(
                    job_id = job.id,
                    subject_id = job.bangumi_subject_id,
                    candidate_id,
                    priority,
                    error = %error,
                    "Candidate activation failed; trying next scored candidate"
                );
                last_error = Some(error);
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        AppError::bad_request("no scored candidate with reachable peers was available")
    }))
}

async fn build_airing_download_plan(
    pool: &SqlitePool,
    job: &crate::types::DownloadJobDto,
    policy: &crate::types::PolicyDto,
    candidates: &[ResourceCandidateDto],
    targets: Option<&AiringEpisodeTargets>,
) -> Result<DownloadPlan, AppError> {
    let eligible = candidates
        .iter()
        .filter(|candidate| {
            candidate.rejected_reason.is_none()
                && !candidate.is_collection
                && candidate.episode_index.is_some()
        })
        .collect::<Vec<_>>();

    let availability = db::list_subject_episode_availability(pool, job.bangumi_subject_id).await?;
    let current_selected = db::current_selected_candidate_for_job(pool, job.id).await?;
    let previous_selected =
        db::latest_selected_candidate_for_subject(pool, job.bangumi_subject_id).await?;

    let mut candidates_by_episode = BTreeMap::<i64, Vec<&ResourceCandidateDto>>::new();
    for candidate in eligible {
        let Some(episode_index) = candidate.episode_index else {
            continue;
        };
        candidates_by_episode
            .entry(episode_sort_key(episode_index))
            .or_default()
            .push(candidate);
    }

    let latest_episode_key = targets
        .and_then(|targets| targets.latest.map(episode_sort_key))
        .or_else(|| candidates_by_episode.keys().next_back().copied());
    let backlog_episodes = targets
        .map(|targets| targets.backlog.clone())
        .unwrap_or_else(|| {
            candidates_by_episode
                .keys()
                .copied()
                .filter(|episode_key| Some(*episode_key) != latest_episode_key)
                .map(|episode_key| episode_key as f64 / 100.0)
                .collect()
        });

    if candidates_by_episode.is_empty()
        && latest_episode_key.is_none()
        && backlog_episodes.is_empty()
    {
        return Ok(DownloadPlan::default());
    }

    let mut preferred_fansub = previous_selected
        .as_ref()
        .and_then(|candidate| candidate.fansub_name.clone());
    let mut backlog_candidate_ids = Vec::new();

    for episode_number in backlog_episodes {
        let episode_key = episode_sort_key(episode_number);
        let Some(slot_candidates) = candidates_by_episode.get(&episode_key) else {
            tracing::info!(
                job_id = job.id,
                subject_id = job.bangumi_subject_id,
                episode = episode_number,
                "No resource candidates matched backlog episode target"
            );
            continue;
        };

        let slot_key = slot_candidates[0].slot_key.as_str();
        let already_covered = availability
            .iter()
            .any(|item| availability_covers_episode(item, episode_number));
        let has_active_execution = db::find_active_execution_for_job_slot(pool, job.id, slot_key)
            .await?
            .is_some();

        if already_covered || has_active_execution {
            tracing::info!(
                job_id = job.id,
                subject_id = job.bangumi_subject_id,
                episode = episode_number,
                slot_key,
                already_covered,
                has_active_execution,
                "Skipping backlog episode target because it is already covered or active"
            );
            continue;
        }

        let ordered_candidates = ordered_slot_candidates(
            slot_candidates.as_slice(),
            &job.release_status,
            preferred_fansub.as_deref(),
        );
        let Some(chosen) = ordered_candidates.first().copied() else {
            tracing::info!(
                job_id = job.id,
                subject_id = job.bangumi_subject_id,
                episode = episode_number,
                slot_key,
                "No eligible candidate remained for backlog episode target after scoring"
            );
            continue;
        };

        tracing::info!(
            job_id = job.id,
            subject_id = job.bangumi_subject_id,
            episode = episode_number,
            candidate_id = chosen.id,
            score = chosen.score,
            fansub = ?chosen.fansub_name,
            title = %chosen.title,
            "Selected backlog episode candidate"
        );
        if let Some(fansub_name) = chosen.fansub_name.clone() {
            preferred_fansub = Some(fansub_name);
        }
        backlog_candidate_ids.push(
            ordered_candidates
                .iter()
                .map(|candidate| candidate.id)
                .collect(),
        );
    }

    let latest_candidates = latest_episode_key
        .and_then(|latest_episode_key| candidates_by_episode.get(&latest_episode_key))
        .and_then(|latest_candidates| {
            if job.release_status == "airing" {
                choose_latest_airing_candidates(
                    job,
                    policy,
                    current_selected.as_ref(),
                    latest_candidates.as_slice(),
                    preferred_fansub.as_deref(),
                )
            } else {
                Some(ordered_slot_candidates(
                    latest_candidates.as_slice(),
                    &job.release_status,
                    preferred_fansub.as_deref(),
                ))
            }
        });

    if let Some(candidate) = latest_candidates
        .as_ref()
        .and_then(|candidates| candidates.first().copied())
    {
        tracing::info!(
            job_id = job.id,
            subject_id = job.bangumi_subject_id,
            episode = candidate.episode_index,
            candidate_id = candidate.id,
            score = candidate.score,
            fansub = ?candidate.fansub_name,
            title = %candidate.title,
            "Selected latest airing episode candidate"
        );
    } else if let Some(latest_episode_key) = latest_episode_key {
        tracing::info!(
            job_id = job.id,
            subject_id = job.bangumi_subject_id,
            latest_episode = latest_episode_key as f64 / 100.0,
            "No latest airing episode candidate was selected"
        );
    }

    let mut candidate_chains = backlog_candidate_ids;
    let latest_candidate_chain = latest_candidates
        .unwrap_or_default()
        .into_iter()
        .map(|candidate| candidate.id)
        .collect::<Vec<_>>();
    if !latest_candidate_chain.is_empty() {
        candidate_chains.push(latest_candidate_chain);
    }

    Ok(DownloadPlan { candidate_chains })
}

async fn build_completed_download_plan(
    pool: &SqlitePool,
    job: &crate::types::DownloadJobDto,
    policy: &crate::types::PolicyDto,
    candidates: &[ResourceCandidateDto],
    targets: Option<&AiringEpisodeTargets>,
) -> Result<DownloadPlan, AppError> {
    let availability = db::list_subject_episode_availability(pool, job.bangumi_subject_id).await?;
    let previous_selected =
        db::latest_selected_candidate_for_subject(pool, job.bangumi_subject_id).await?;
    let tracked_episodes = targets
        .map(AiringEpisodeTargets::search_targets)
        .unwrap_or_default();
    let missing_episodes = tracked_episodes
        .iter()
        .copied()
        .filter(|episode| {
            !availability
                .iter()
                .any(|item| availability_covers_episode(item, *episode))
        })
        .collect::<Vec<_>>();

    let preferred_fansub = previous_selected
        .as_ref()
        .and_then(|candidate| candidate.fansub_name.as_deref());
    let collection_candidates = candidates
        .iter()
        .filter(|candidate| candidate.rejected_reason.is_none() && candidate.is_collection)
        .collect::<Vec<_>>();

    let already_has_any_media = availability
        .iter()
        .any(|item| item.status == "ready" || item.status == "partial");
    if !already_has_any_media && !missing_episodes.is_empty() && !collection_candidates.is_empty() {
        let eligible_collection_candidates = ordered_slot_candidates(
            &collection_candidates
                .iter()
                .copied()
                .filter(|candidate| collection_matches_target_window(candidate, &missing_episodes))
                .collect::<Vec<_>>(),
            &job.release_status,
            preferred_fansub,
        );
        if !eligible_collection_candidates.is_empty() {
            tracing::info!(
                job_id = job.id,
                subject_id = job.bangumi_subject_id,
                missing_episodes = ?missing_episodes,
                candidate_id = eligible_collection_candidates[0].id,
                title = %eligible_collection_candidates[0].title,
                score = eligible_collection_candidates[0].score,
                "Selected completed-season collection candidate chain"
            );
            return Ok(DownloadPlan {
                candidate_chains: vec![
                    eligible_collection_candidates
                        .into_iter()
                        .map(|candidate| candidate.id)
                        .collect(),
                ],
            });
        }
    }

    let eligible = candidates
        .iter()
        .filter(|candidate| {
            candidate.rejected_reason.is_none()
                && !candidate.is_collection
                && candidate.episode_index.is_some()
        })
        .collect::<Vec<_>>();
    let mut candidates_by_episode = BTreeMap::<i64, Vec<&ResourceCandidateDto>>::new();
    for candidate in eligible {
        let Some(episode_index) = candidate.episode_index else {
            continue;
        };
        candidates_by_episode
            .entry(episode_sort_key(episode_index))
            .or_default()
            .push(candidate);
    }

    let mut preferred_fansub = preferred_fansub.map(str::to_owned);
    let mut candidate_chains = Vec::new();
    for episode_number in tracked_episodes {
        if availability
            .iter()
            .any(|item| availability_covers_episode(item, episode_number))
        {
            continue;
        }

        let episode_key = episode_sort_key(episode_number);
        let Some(slot_candidates) = candidates_by_episode.get(&episode_key) else {
            tracing::info!(
                job_id = job.id,
                subject_id = job.bangumi_subject_id,
                episode = episode_number,
                "No completed-season resource candidates matched target episode"
            );
            continue;
        };
        let slot_key = slot_candidates[0].slot_key.as_str();
        if db::find_active_execution_for_job_slot(pool, job.id, slot_key)
            .await?
            .is_some()
        {
            continue;
        }

        let ordered_candidates = ordered_slot_candidates(
            slot_candidates.as_slice(),
            &job.release_status,
            preferred_fansub.as_deref(),
        );
        let Some(chosen) = ordered_candidates.first().copied() else {
            continue;
        };
        tracing::info!(
            job_id = job.id,
            subject_id = job.bangumi_subject_id,
            episode = episode_number,
            candidate_id = chosen.id,
            score = chosen.score,
            fansub = ?chosen.fansub_name,
            title = %chosen.title,
            "Selected completed-season episode candidate"
        );
        if let Some(fansub_name) = chosen.fansub_name.clone() {
            preferred_fansub = Some(fansub_name);
        }
        candidate_chains.push(
            ordered_candidates
                .into_iter()
                .map(|candidate| candidate.id)
                .collect(),
        );
    }

    let _ = policy;
    Ok(DownloadPlan { candidate_chains })
}

fn collection_covers_all_targets(candidate: &ResourceCandidateDto, targets: &[f64]) -> bool {
    !targets.is_empty()
        && targets
            .iter()
            .all(|episode| candidate_covers_episode(candidate, *episode))
}

fn collection_matches_target_window(candidate: &ResourceCandidateDto, targets: &[f64]) -> bool {
    if !collection_covers_all_targets(candidate, targets) {
        return false;
    }

    let Some(start) = candidate.episode_index else {
        return false;
    };
    let end = candidate.episode_end_index.unwrap_or(start);
    let min_target = targets
        .iter()
        .copied()
        .min_by(|left, right| left.total_cmp(right))
        .unwrap_or(start);
    let max_target = targets
        .iter()
        .copied()
        .max_by(|left, right| left.total_cmp(right))
        .unwrap_or(end);

    start >= min_target - 1.0 && end <= max_target + 1.0
}

fn choose_latest_airing_candidates<'a>(
    job: &crate::types::DownloadJobDto,
    policy: &crate::types::PolicyDto,
    current_selected: Option<&'a ResourceCandidateDto>,
    latest_candidates: &[&'a ResourceCandidateDto],
    preferred_fansub: Option<&str>,
) -> Option<Vec<&'a ResourceCandidateDto>> {
    let ordered = ordered_slot_candidates(latest_candidates, &job.release_status, preferred_fansub);
    let best = ordered.first().copied()?;

    let Some(current) = current_selected else {
        return Some(ordered);
    };

    if current.slot_key != best.slot_key {
        return Some(ordered);
    }

    if !within_replacement_window(
        job.selection_updated_at.as_deref(),
        policy.replacement_window_hours,
    ) {
        return Some(vec![current]);
    }

    if slot_candidate_priority_key(best, &job.release_status, preferred_fansub)
        > slot_candidate_priority_key(current, &job.release_status, preferred_fansub)
    {
        Some(ordered)
    } else {
        Some(vec![current])
    }
}

fn ordered_slot_candidates<'a>(
    candidates: &[&'a ResourceCandidateDto],
    release_status: &str,
    preferred_fansub: Option<&str>,
) -> Vec<&'a ResourceCandidateDto> {
    let mut ordered = candidates.to_vec();
    ordered.sort_by(|left, right| {
        slot_candidate_priority_key(right, release_status, preferred_fansub).cmp(
            &slot_candidate_priority_key(left, release_status, preferred_fansub),
        )
    });
    ordered
}

fn slot_candidate_priority_key(
    candidate: &ResourceCandidateDto,
    release_status: &str,
    preferred_fansub: Option<&str>,
) -> (i64, i64, i64, i64) {
    let (slot_weight, score_weight, quality_weight, freshness_weight) =
        candidate_priority_key(candidate, release_status);
    let continuity_bonus = if preferred_fansub
        .zip(candidate.fansub_name.as_deref())
        .is_some_and(|(left, right)| normalize_fansub_name(left) == normalize_fansub_name(right))
    {
        24_000
    } else {
        0
    };

    (
        slot_weight,
        score_weight + continuity_bonus,
        quality_weight,
        freshness_weight,
    )
}

fn normalize_fansub_name(value: &str) -> String {
    value
        .chars()
        .filter(|character| {
            !character.is_whitespace() && !matches!(character, '(' | ')' | '[' | ']')
        })
        .flat_map(char::to_lowercase)
        .collect()
}

fn episode_sort_key(value: f64) -> i64 {
    (value * 100.0).round() as i64
}

async fn fetch_subject_card_map(
    bangumi: &BangumiClient,
    yuc: &YucClient,
    subject_ids: &[i64],
) -> HashMap<i64, SubjectCardDto> {
    let unique_ids = subject_ids
        .iter()
        .copied()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    stream::iter(unique_ids.into_iter().map(|subject_id| {
        let bangumi = bangumi.clone();
        let yuc = yuc.clone();
        async move {
            match bangumi.fetch_subject(subject_id).await {
                Ok(subject) => Some((subject_id, yuc.enrich_card(subject.to_card()).await)),
                Err(error) => {
                    tracing::warn!(
                        subject_id,
                        error = %error,
                        "Failed to fetch Bangumi subject card for user collection"
                    );
                    None
                }
            }
        }
    }))
    .buffer_unordered(8)
    .filter_map(|item| async move { item })
    .collect::<HashMap<_, _>>()
    .await
}

#[derive(Debug, Clone)]
struct HydratedSubscriptionItem {
    card: SubjectCardDto,
    subscribed_at: String,
    latest_ready_at: Option<String>,
}

async fn hydrate_subscription_cards(
    bangumi: &BangumiClient,
    yuc: &YucClient,
    entries: Vec<db::ViewerSubscriptionEntry>,
    keyword: &str,
) -> Vec<HydratedSubscriptionItem> {
    let card_map = fetch_subject_card_map(
        bangumi,
        yuc,
        &entries
            .iter()
            .map(|entry| entry.bangumi_subject_id)
            .collect::<Vec<_>>(),
    )
    .await;
    let keyword = keyword.trim().to_lowercase();

    entries
        .into_iter()
        .filter_map(|entry| {
            let card = card_map.get(&entry.bangumi_subject_id)?.clone();
            if !keyword.is_empty() {
                let title = card.title.to_lowercase();
                let title_cn = card.title_cn.to_lowercase();
                if !title.contains(&keyword) && !title_cn.contains(&keyword) {
                    return None;
                }
            }

            Some(HydratedSubscriptionItem {
                card,
                subscribed_at: entry.subscribed_at,
                latest_ready_at: entry.latest_ready_at,
            })
        })
        .collect()
}

fn sort_subscription_items(items: &mut [HydratedSubscriptionItem], sort: &str) {
    match sort {
        "rating" => items.sort_by(|left, right| {
            let left_score = left.card.rating_score.unwrap_or(-1.0);
            let right_score = right.card.rating_score.unwrap_or(-1.0);
            right_score
                .total_cmp(&left_score)
                .then_with(|| left.card.title_cn.cmp(&right.card.title_cn))
                .then_with(|| left.card.title.cmp(&right.card.title))
        }),
        "title" => items.sort_by(|left, right| {
            left.card
                .title_cn
                .cmp(&right.card.title_cn)
                .then_with(|| left.card.title.cmp(&right.card.title))
        }),
        _ => items.sort_by(|left, right| {
            let left_key = left
                .latest_ready_at
                .as_deref()
                .unwrap_or(left.subscribed_at.as_str());
            let right_key = right
                .latest_ready_at
                .as_deref()
                .unwrap_or(right.subscribed_at.as_str());

            right_key
                .cmp(left_key)
                .then_with(|| left.card.title_cn.cmp(&right.card.title_cn))
                .then_with(|| left.card.title.cmp(&right.card.title))
        }),
    }
}

#[derive(Debug, Clone)]
struct SubjectHistoryMetadata {
    detail: SubjectDetailDto,
    episodes: Vec<EpisodeRaw>,
}

async fn fetch_subject_history_metadata_map(
    bangumi: &BangumiClient,
    yuc: &YucClient,
    subject_ids: &[i64],
) -> HashMap<i64, SubjectHistoryMetadata> {
    let unique_ids = subject_ids
        .iter()
        .copied()
        .collect::<HashSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    stream::iter(unique_ids.into_iter().map(|subject_id| {
        let bangumi = bangumi.clone();
        let yuc = yuc.clone();
        async move {
            let subject = match bangumi.fetch_subject(subject_id).await {
                Ok(subject) => subject,
                Err(error) => {
                    tracing::warn!(
                        subject_id,
                        error = %error,
                        "Failed to fetch Bangumi subject for playback history"
                    );
                    return None;
                }
            };
            let episodes = match bangumi.fetch_episodes(subject_id).await {
                Ok(episodes) => episodes,
                Err(error) => {
                    tracing::warn!(
                        subject_id,
                        error = %error,
                        "Failed to fetch Bangumi episodes for playback history"
                    );
                    return None;
                }
            };

            Some((
                subject_id,
                SubjectHistoryMetadata {
                    detail: yuc.enrich_detail(subject.to_detail()).await,
                    episodes,
                },
            ))
        }
    }))
    .buffer_unordered(6)
    .filter_map(|item| async move { item })
    .collect::<HashMap<_, _>>()
    .await
}

async fn hydrate_playback_history(
    bangumi: &BangumiClient,
    yuc: &YucClient,
    entries: Vec<db::PlaybackHistoryEntry>,
) -> Vec<PlaybackHistoryItemDto> {
    let metadata = fetch_subject_history_metadata_map(
        bangumi,
        yuc,
        &entries
            .iter()
            .map(|entry| entry.bangumi_subject_id)
            .collect::<Vec<_>>(),
    )
    .await;

    entries
        .into_iter()
        .filter_map(|entry| {
            let metadata = metadata.get(&entry.bangumi_subject_id)?;
            let episode = metadata
                .episodes
                .iter()
                .find(|episode| episode.id == entry.bangumi_episode_id);

            Some(PlaybackHistoryItemDto {
                bangumi_subject_id: entry.bangumi_subject_id,
                bangumi_episode_id: entry.bangumi_episode_id,
                episode_number: episode.and_then(EpisodeRaw::preferred_episode_number),
                subject_title: metadata.detail.title.clone(),
                subject_title_cn: metadata.detail.title_cn.clone(),
                episode_title: episode.map(|item| item.name.clone()).unwrap_or_default(),
                episode_title_cn: episode.map(|item| item.name_cn.clone()).unwrap_or_default(),
                image_portrait: metadata.detail.image_portrait.clone(),
                file_name: entry.file_name,
                source_fansub_name: entry.source_fansub_name,
                last_played_at: entry.last_played_at,
                play_count: entry.play_count,
            })
        })
        .collect()
}

async fn hydrate_active_downloads(
    bangumi: &BangumiClient,
    yuc: &YucClient,
    executions: Vec<crate::types::DownloadExecutionDto>,
) -> Vec<ActiveDownloadDto> {
    let card_map = fetch_subject_card_map(
        bangumi,
        yuc,
        &executions
            .iter()
            .map(|execution| execution.bangumi_subject_id)
            .collect::<Vec<_>>(),
    )
    .await;

    executions
        .into_iter()
        .map(|execution| {
            let fallback_title = execution.source_title.clone();
            let card = card_map.get(&execution.bangumi_subject_id);
            let release_status = card
                .map(|item| item.release_status.clone())
                .unwrap_or_else(|| "completed".to_owned());
            let (slot_key, episode_index, episode_end_index, is_collection) =
                sanitize_download_display_slot(
                    &execution.source_title,
                    &release_status,
                    execution.is_collection,
                    execution.slot_key.clone(),
                    execution.episode_index,
                    execution.episode_end_index,
                );
            ActiveDownloadDto {
                bangumi_subject_id: execution.bangumi_subject_id,
                title: card
                    .map(|item| item.title.clone())
                    .unwrap_or_else(|| fallback_title.clone()),
                title_cn: card.map(|item| item.title_cn.clone()).unwrap_or_default(),
                image_portrait: card.and_then(|item| item.image_portrait.clone()),
                release_status,
                slot_key,
                episode_index,
                episode_end_index,
                is_collection,
                state: execution.state,
                source_title: execution.source_title,
                source_fansub_name: execution.source_fansub_name,
                downloaded_bytes: execution.downloaded_bytes,
                total_bytes: execution.source_size_bytes.max(execution.downloaded_bytes),
                download_rate_bytes: execution.download_rate_bytes,
                upload_rate_bytes: execution.upload_rate_bytes,
                peer_count: execution.peer_count,
                updated_at: execution.updated_at,
            }
        })
        .collect()
}

fn sanitize_download_display_slot(
    source_title: &str,
    release_status: &str,
    is_collection: bool,
    slot_key: String,
    episode_index: Option<f64>,
    episode_end_index: Option<f64>,
) -> (String, Option<f64>, Option<f64>, bool) {
    let has_negative = episode_index.is_some_and(|value| value < 0.0)
        || episode_end_index.is_some_and(|value| value < 0.0);
    let has_inverted_range = episode_index
        .zip(episode_end_index)
        .is_some_and(|(start, end)| end < start);
    if !has_negative && !has_inverted_range {
        return (slot_key, episode_index, episode_end_index, is_collection);
    }

    let reparsed = media::infer_release_slot(
        source_title,
        if is_collection { "batch" } else { "single" },
        &slot_key,
        release_status,
    );
    let reparsed_valid = reparsed.episode_index.is_some_and(|value| value >= 0.0)
        && reparsed
            .episode_end_index
            .unwrap_or(reparsed.episode_index.unwrap_or(0.0))
            >= reparsed.episode_index.unwrap_or(0.0);
    if reparsed_valid {
        return (
            reparsed.slot_key,
            reparsed.episode_index,
            reparsed.episode_end_index,
            reparsed.is_collection,
        );
    }

    let fallback_start = episode_index.unwrap_or(0.0).max(0.0);
    let fallback_end = episode_end_index
        .unwrap_or(fallback_start)
        .max(fallback_start);
    let sanitized_slot_key = if is_collection {
        format!(
            "batch:{}-{}",
            format_download_slot_fragment(fallback_start),
            format_download_slot_fragment(fallback_end)
        )
    } else {
        format!("episode:{}", format_download_slot_fragment(fallback_start))
    };

    (
        sanitized_slot_key,
        Some(fallback_start),
        Some(fallback_end),
        is_collection,
    )
}

fn format_download_slot_fragment(value: f64) -> String {
    if value.fract().abs() < f64::EPSILON {
        format!("{}", value.round() as i64)
    } else {
        format!("{value:.1}")
    }
}

#[derive(Debug, Clone)]
struct AnimeGardenSearchProfileWithStatus {
    bangumi_subject_id: i64,
    title: String,
    title_cn: String,
    release_status: String,
    season_hint: Option<i64>,
}

impl AnimeGardenSearchProfileWithStatus {
    fn to_discovery_profile(&self) -> AnimeGardenSearchProfile {
        AnimeGardenSearchProfile {
            bangumi_subject_id: self.bangumi_subject_id,
            title: self.title.clone(),
            title_cn: self.title_cn.clone(),
            season_hint: self.season_hint,
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

fn normalize_collection_sort(sort: Option<&str>) -> String {
    match sort.unwrap_or("updated") {
        "updated" | "rating" | "title" => sort.unwrap_or("updated").to_owned(),
        _ => "updated".to_owned(),
    }
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

fn episode_has_aired(episode: &EpisodeRaw) -> bool {
    if episode.airdate.trim().is_empty() {
        return true;
    }

    let date_part = episode
        .airdate
        .split_once('T')
        .map(|(left, _)| left)
        .unwrap_or(episode.airdate.as_str());
    let Ok(airdate) = NaiveDate::parse_from_str(date_part, "%Y-%m-%d") else {
        return true;
    };

    airdate <= tokyo_today()
}

fn tokyo_today() -> NaiveDate {
    Utc::now()
        .with_timezone(&FixedOffset::east_opt(9 * 3600).expect("valid tokyo utc offset"))
        .date_naive()
}

fn episode_already_tracked(
    episode_number: f64,
    availability: &[db::SubjectEpisodeAvailability],
    executions: &[crate::types::DownloadExecutionDto],
) -> bool {
    availability
        .iter()
        .any(|item| availability_covers_episode(item, episode_number))
        || executions.iter().any(|execution| {
            matches!(
                execution.state.as_str(),
                "staged" | "starting" | "downloading" | "completed" | "seeding"
            ) && execution_covers_episode(execution, episode_number)
        })
}

fn candidate_covers_episode(candidate: &ResourceCandidateDto, episode_number: f64) -> bool {
    numeric_range_covers_episode(
        candidate.episode_index,
        candidate.episode_end_index,
        candidate.is_collection,
        episode_number,
    )
}

fn execution_covers_episode(
    execution: &crate::types::DownloadExecutionDto,
    episode_number: f64,
) -> bool {
    numeric_range_covers_episode(
        execution.episode_index,
        execution.episode_end_index,
        execution.is_collection,
        episode_number,
    )
}

fn availability_covers_episode(item: &db::SubjectEpisodeAvailability, episode_number: f64) -> bool {
    numeric_range_covers_episode(
        item.episode_index,
        item.episode_end_index,
        item.is_collection,
        episode_number,
    )
}

fn numeric_range_covers_episode(
    start: Option<f64>,
    end: Option<f64>,
    is_collection: bool,
    episode_number: f64,
) -> bool {
    let Some(start) = start else {
        return false;
    };
    let end = end.unwrap_or(start);
    let epsilon = if is_collection { 0.001 } else { f64::EPSILON };

    episode_number + epsilon >= start && episode_number - epsilon <= end
}

fn format_runtime_duration(duration: std::time::Duration) -> String {
    let total = duration.as_secs();
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

#[cfg(test)]
mod tests {
    use super::collection_matches_target_window;
    use crate::types::ResourceCandidateDto;

    fn sample_collection_candidate(start: f64, end: f64) -> ResourceCandidateDto {
        ResourceCandidateDto {
            id: 1,
            download_job_id: 1,
            search_run_id: 1,
            bangumi_subject_id: 1,
            slot_key: format!("batch:{start}-{end}"),
            episode_index: Some(start),
            episode_end_index: Some(end),
            is_collection: true,
            provider: "animegarden".to_owned(),
            provider_resource_id: "sample".to_owned(),
            title: "sample".to_owned(),
            href: String::new(),
            magnet: String::new(),
            release_type: "batch".to_owned(),
            size_bytes: 1,
            fansub_name: Some("sample".to_owned()),
            publisher_name: "sample".to_owned(),
            source_created_at: "2026-01-01T00:00:00Z".to_owned(),
            source_fetched_at: "2026-01-01T00:00:00Z".to_owned(),
            resolution: Some("1080P".to_owned()),
            locale_hint: Some("zh-Hans".to_owned()),
            is_raw: false,
            score: 10.0,
            rejected_reason: None,
            discovered_at: "2026-01-01T00:00:00Z".to_owned(),
        }
    }

    #[test]
    fn completed_collection_window_rejects_oversized_franchise_batches() {
        let targets = (1..=11).map(|value| value as f64).collect::<Vec<_>>();
        assert!(!collection_matches_target_window(
            &sample_collection_candidate(1.0, 24.0),
            &targets,
        ));
        assert!(collection_matches_target_window(
            &sample_collection_candidate(0.0, 11.0),
            &targets,
        ));
    }
}
