use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    num::NonZeroU32,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, anyhow};
use axum::{
    Json, Router,
    extract::{Path as AxumPath, Query, State},
    http::StatusCode,
    routing::{get, patch, post},
};
use chrono::{DateTime, Utc};
use librqbit::{
    AddTorrent, AddTorrentOptions, Api as RqbitApi, Session, SessionOptions,
    SessionPersistenceConfig, TorrentStats, TorrentStatsState,
    api::{ApiAddTorrentResponse, TorrentIdOrHash},
    limits::LimitsConfig,
};
use serde::{Deserialize, Serialize};
use tokio::{
    sync::{Mutex, RwLock},
    task::JoinHandle,
    time::sleep,
};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    config::DownloaderConfig,
    model::{
        CreateTaskRequest, CreateTaskResponse, DownloaderTaskDto, InspectTaskRequest,
        RuntimeOverviewResponse, RuntimeSettingsDto, TaskKind, TaskListResponse, TaskSource,
        TaskSourceKind, TaskState, TorrentFileEntry, TorrentMetadataSummary, UpdateSettingsRequest,
        UpdateTaskRequest,
    },
};

#[derive(Clone)]
pub struct DownloaderService {
    config: Arc<RwLock<DownloaderConfig>>,
    tasks: Arc<RwLock<HashMap<Uuid, TaskRecord>>>,
    sessions: Arc<Mutex<HashMap<Uuid, TaskSession>>>,
    started_at: DateTime<Utc>,
}

pub struct DownloaderRuntime {
    service: Arc<DownloaderService>,
    scheduler: JoinHandle<()>,
}

impl DownloaderRuntime {
    pub fn service(&self) -> Arc<DownloaderService> {
        self.service.clone()
    }

    pub fn abort(self) {
        self.scheduler.abort();
    }
}

pub fn start_embedded(config: DownloaderConfig) -> anyhow::Result<DownloaderRuntime> {
    let service = Arc::new(DownloaderService::new(config)?);
    let scheduler = service.clone().spawn_scheduler();
    Ok(DownloaderRuntime { service, scheduler })
}

#[derive(Debug, Clone)]
struct TaskRecord {
    id: Uuid,
    kind: TaskKind,
    state: TaskState,
    enabled: bool,
    priority: u32,
    seed_after_download: bool,
    source: TaskSource,
    output_dir: String,
    display_name: Option<String>,
    info_hash: Option<String>,
    metadata: Option<TorrentMetadataSummary>,
    engine_id: Option<String>,
    downloaded_bytes: u64,
    total_bytes: u64,
    uploaded_bytes: u64,
    download_rate_bytes: u64,
    upload_rate_bytes: u64,
    peer_count: u32,
    manual_download_limit_mb: Option<u64>,
    manual_upload_limit_mb: Option<u64>,
    effective_download_limit_bps: Option<u64>,
    effective_upload_limit_bps: Option<u64>,
    stall_timeout_secs: u64,
    total_timeout_secs: u64,
    started_at: Option<DateTime<Utc>>,
    completed_at: Option<DateTime<Utc>>,
    last_progress_at: Option<DateTime<Utc>>,
    last_progress_bytes: u64,
    last_error: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TaskRecord {
    fn new(
        request: CreateTaskRequest,
        metadata: TorrentMetadataSummary,
        defaults: &DownloaderConfig,
    ) -> Self {
        let now = Utc::now();
        let enabled = request.start_enabled.unwrap_or(true);
        let state = if enabled {
            TaskState::Queued
        } else {
            TaskState::Paused
        };
        let output_dir = request
            .output_dir
            .clone()
            .unwrap_or_else(|| defaults.default_output_dir.to_string_lossy().into_owned());

        Self {
            id: Uuid::new_v4(),
            kind: request.kind,
            state,
            enabled,
            priority: request.priority.unwrap_or(0),
            seed_after_download: request.seed_after_download.unwrap_or(true),
            source: request.source,
            output_dir,
            display_name: metadata.name.clone(),
            info_hash: Some(metadata.info_hash.clone()),
            metadata: Some(metadata),
            engine_id: None,
            downloaded_bytes: 0,
            total_bytes: 0,
            uploaded_bytes: 0,
            download_rate_bytes: 0,
            upload_rate_bytes: 0,
            peer_count: 0,
            manual_download_limit_mb: request.manual_download_limit_mb,
            manual_upload_limit_mb: request.manual_upload_limit_mb,
            effective_download_limit_bps: None,
            effective_upload_limit_bps: None,
            stall_timeout_secs: request
                .stall_timeout_secs
                .unwrap_or(defaults.stall_timeout_secs),
            total_timeout_secs: request
                .total_timeout_secs
                .unwrap_or(defaults.total_timeout_secs),
            started_at: None,
            completed_at: None,
            last_progress_at: None,
            last_progress_bytes: 0,
            last_error: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn to_dto(&self, queue_position: Option<usize>) -> DownloaderTaskDto {
        DownloaderTaskDto {
            id: self.id,
            kind: self.kind.clone(),
            state: self.state.clone(),
            enabled: self.enabled,
            priority: self.priority,
            queue_position,
            seed_after_download: self.seed_after_download,
            source: self.source.clone(),
            output_dir: self.output_dir.clone(),
            display_name: self.display_name.clone(),
            info_hash: self.info_hash.clone(),
            metadata: self.metadata.clone(),
            engine_id: self.engine_id.clone(),
            downloaded_bytes: self.downloaded_bytes,
            total_bytes: self.total_bytes,
            uploaded_bytes: self.uploaded_bytes,
            download_rate_bytes: self.download_rate_bytes,
            upload_rate_bytes: self.upload_rate_bytes,
            peer_count: self.peer_count,
            manual_download_limit_mb: self.manual_download_limit_mb,
            manual_upload_limit_mb: self.manual_upload_limit_mb,
            effective_download_limit_mb: self
                .effective_download_limit_bps
                .map(bytes_per_second_to_mb_per_second),
            effective_upload_limit_mb: self
                .effective_upload_limit_bps
                .map(bytes_per_second_to_mb_per_second),
            stall_timeout_secs: self.stall_timeout_secs,
            total_timeout_secs: self.total_timeout_secs,
            started_at: self.started_at,
            completed_at: self.completed_at,
            last_progress_at: self.last_progress_at,
            last_error: self.last_error.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

struct TaskSession {
    session: Arc<Session>,
    api: RqbitApi,
    torrent_ref: TorrentIdOrHash,
}

impl TaskSession {
    async fn pause(&self) -> anyhow::Result<()> {
        self.api
            .api_torrent_action_pause(self.torrent_ref)
            .await
            .map(|_| ())
            .map_err(|error| anyhow!(error.to_string()))
    }

    async fn resume(&self) -> anyhow::Result<()> {
        self.api
            .api_torrent_action_start(self.torrent_ref)
            .await
            .map(|_| ())
            .map_err(|error| anyhow!(error.to_string()))
    }

    async fn delete(&self, delete_files: bool) -> anyhow::Result<()> {
        if delete_files {
            self.api
                .api_torrent_action_delete(self.torrent_ref)
                .await
                .map(|_| ())
                .map_err(|error| anyhow!(error.to_string()))
        } else {
            self.api
                .api_torrent_action_forget(self.torrent_ref)
                .await
                .map(|_| ())
                .map_err(|error| anyhow!(error.to_string()))
        }
    }

    fn stats(&self) -> anyhow::Result<TorrentStats> {
        self.api
            .api_stats_v1(self.torrent_ref)
            .map_err(|error| anyhow!(error.to_string()))
    }

    fn apply_limits(&self, download_bps: Option<u64>, upload_bps: Option<u64>) {
        self.session
            .ratelimits
            .set_download_bps(limit_to_non_zero(download_bps));
        self.session
            .ratelimits
            .set_upload_bps(limit_to_non_zero(upload_bps));
    }
}

#[derive(Serialize)]
struct ApiEnvelope<T> {
    data: T,
}

#[derive(Serialize)]
struct ErrorPayload {
    message: String,
}

#[derive(Debug, Clone, Copy)]
enum QueueCategory {
    Download,
    Seed,
}

#[derive(Debug, Clone)]
struct QueuePlan {
    active_downloads: Vec<Uuid>,
    active_seeds: Vec<Uuid>,
    queue_positions: HashMap<Uuid, usize>,
    download_limits: HashMap<Uuid, Option<u64>>,
    upload_limits: HashMap<Uuid, Option<u64>>,
}

#[derive(Debug, Clone, Default)]
struct SchedulerSummary {
    active_downloads: usize,
    active_seeds: usize,
    queued_downloads: usize,
    queued_seeds: usize,
}

#[derive(Debug, Clone)]
struct TaskRuntimeSnapshot {
    state: TaskState,
    downloaded_bytes: u64,
    total_bytes: u64,
    uploaded_bytes: u64,
    download_rate_bytes: u64,
    upload_rate_bytes: u64,
    peer_count: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct DeleteTaskQuery {
    delete_files: Option<bool>,
}

impl DownloaderService {
    pub fn new(config: DownloaderConfig) -> anyhow::Result<Self> {
        fs::create_dir_all(&config.runtime_root).with_context(|| {
            format!(
                "failed to create downloader runtime root {}",
                config.runtime_root.display()
            )
        })?;

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            tasks: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            started_at: Utc::now(),
        })
    }

    pub fn spawn_scheduler(self: Arc<Self>) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                if let Err(error) = self.scheduler_tick().await {
                    warn!(error = %error, "Downloader scheduler tick failed");
                }

                let interval_secs = self.config.read().await.scheduler_interval_secs;
                sleep(Duration::from_secs(interval_secs)).await;
            }
        })
    }

    pub async fn create_task(
        &self,
        mut request: CreateTaskRequest,
    ) -> anyhow::Result<CreateTaskResponse> {
        let defaults = self.config.read().await.clone();
        if request.output_dir.is_none() {
            request.output_dir = Some(defaults.default_output_dir.to_string_lossy().into_owned());
        }
        let metadata = self
            .inspect_source(InspectTaskRequest {
                source: request.source.clone(),
                output_dir: request.output_dir.clone(),
            })
            .await?;

        {
            let tasks = self.tasks.read().await;
            if let Some(existing) = tasks.values().find(|task| {
                !matches!(task.state, TaskState::Deleted)
                    && task.info_hash.as_deref() == Some(metadata.info_hash.as_str())
            }) {
                let existing_id = existing.id;
                drop(tasks);
                return Ok(CreateTaskResponse {
                    task: self.get_task(existing_id).await?,
                    created: false,
                });
            }
        }

        let record = TaskRecord::new(request, metadata, &defaults);
        let dto = record.to_dto(None);
        self.tasks.write().await.insert(record.id, record);
        Ok(CreateTaskResponse {
            task: dto,
            created: true,
        })
    }

    pub async fn inspect_source(
        &self,
        request: InspectTaskRequest,
    ) -> anyhow::Result<TorrentMetadataSummary> {
        let config = self.config.read().await.clone();
        let runtime_root = config.runtime_root.clone();
        let inspect_root = runtime_root
            .join("_inspect")
            .join(Uuid::new_v4().to_string());
        let output_dir = request
            .output_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| config.default_output_dir.clone());

        let session = build_session(&inspect_root, &output_dir, None, None).await?;
        let api = RqbitApi::new(session.clone(), None);
        let response = api
            .api_add_torrent(
                add_torrent_from_source(&request.source)?,
                Some(AddTorrentOptions {
                    paused: true,
                    list_only: true,
                    overwrite: true,
                    output_folder: Some(output_dir.to_string_lossy().into_owned()),
                    ..Default::default()
                }),
            )
            .await
            .map_err(|error| anyhow!(error.to_string()))?;

        Ok(metadata_from_add_response(response))
    }

    pub async fn list_all_tasks(&self) -> TaskListResponse {
        self.list_tasks_internal(None).await
    }

    pub async fn list_downloads(&self) -> TaskListResponse {
        self.list_tasks_internal(Some(QueueCategory::Download))
            .await
    }

    pub async fn list_seeds(&self) -> TaskListResponse {
        self.list_tasks_internal(Some(QueueCategory::Seed)).await
    }

    async fn list_tasks_internal(&self, filter: Option<QueueCategory>) -> TaskListResponse {
        let config = self.config.read().await.clone();
        let tasks = self.tasks.read().await;
        let plan = compute_queue_plan(&tasks, &config);
        let items = tasks
            .values()
            .filter(|task| matches_category(task, filter))
            .cloned()
            .map(|task| task.to_dto(plan.queue_positions.get(&task.id).copied()))
            .collect::<Vec<_>>();

        TaskListResponse { items }
    }

    pub async fn get_task(&self, task_id: Uuid) -> anyhow::Result<DownloaderTaskDto> {
        let config = self.config.read().await.clone();
        let tasks = self.tasks.read().await;
        let plan = compute_queue_plan(&tasks, &config);
        let task = tasks
            .get(&task_id)
            .ok_or_else(|| anyhow!("task {task_id} not found"))?;

        Ok(task.to_dto(plan.queue_positions.get(&task.id).copied()))
    }

    pub async fn pause_task(&self, task_id: Uuid) -> anyhow::Result<DownloaderTaskDto> {
        {
            let mut tasks = self.tasks.write().await;
            let task = tasks
                .get_mut(&task_id)
                .ok_or_else(|| anyhow!("task {task_id} not found"))?;
            task.enabled = false;
            task.state = TaskState::Paused;
            task.updated_at = Utc::now();
        }

        if let Some(session) = self.sessions.lock().await.get(&task_id) {
            session.pause().await?;
        }

        self.get_task(task_id).await
    }

    pub async fn resume_task(&self, task_id: Uuid) -> anyhow::Result<DownloaderTaskDto> {
        {
            let mut tasks = self.tasks.write().await;
            let task = tasks
                .get_mut(&task_id)
                .ok_or_else(|| anyhow!("task {task_id} not found"))?;
            task.enabled = true;
            task.state = TaskState::Queued;
            task.last_error = None;
            task.updated_at = Utc::now();
        }

        self.get_task(task_id).await
    }

    pub async fn delete_task(
        &self,
        task_id: Uuid,
        delete_files: bool,
    ) -> anyhow::Result<Option<DownloaderTaskDto>> {
        if let Some(session) = self.sessions.lock().await.remove(&task_id) {
            if let Err(error) = session.delete(delete_files).await {
                warn!(task_id = %task_id, error = %error, "Failed to delete rqbit task session");
            }
        }

        let removed = self.tasks.write().await.remove(&task_id);
        Ok(removed.map(|task| task.to_dto(None)))
    }

    pub async fn update_task(
        &self,
        task_id: Uuid,
        request: UpdateTaskRequest,
    ) -> anyhow::Result<DownloaderTaskDto> {
        let mut tasks = self.tasks.write().await;
        let task = tasks
            .get_mut(&task_id)
            .ok_or_else(|| anyhow!("task {task_id} not found"))?;

        if let Some(priority) = request.priority {
            task.priority = priority;
        }
        if let Some(enabled) = request.enabled {
            task.enabled = enabled;
            if !enabled {
                task.state = TaskState::Paused;
            } else if matches!(task.state, TaskState::Paused | TaskState::Failed) {
                task.state = TaskState::Queued;
            }
        }
        if let Some(seed_after_download) = request.seed_after_download {
            task.seed_after_download = seed_after_download;
        }
        if request.clear_manual_download_limit.unwrap_or(false) {
            task.manual_download_limit_mb = None;
        } else if let Some(value) = request.manual_download_limit_mb {
            task.manual_download_limit_mb = Some(value);
        }
        if request.clear_manual_upload_limit.unwrap_or(false) {
            task.manual_upload_limit_mb = None;
        } else if let Some(value) = request.manual_upload_limit_mb {
            task.manual_upload_limit_mb = Some(value);
        }
        if let Some(value) = request.stall_timeout_secs {
            task.stall_timeout_secs = value.max(60);
        }
        if let Some(value) = request.total_timeout_secs {
            task.total_timeout_secs = value.max(task.stall_timeout_secs);
        }
        task.updated_at = Utc::now();

        Ok(task.to_dto(None))
    }

    pub async fn update_settings(
        &self,
        request: UpdateSettingsRequest,
    ) -> anyhow::Result<RuntimeOverviewResponse> {
        let mut config = self.config.write().await;
        if let Some(value) = request.default_output_dir {
            config.default_output_dir = PathBuf::from(value);
        }
        if let Some(value) = request.max_concurrent_downloads {
            config.max_concurrent_downloads = value.max(1);
        }
        if let Some(value) = request.max_concurrent_seeds {
            config.max_concurrent_seeds = value.max(1);
        }
        if let Some(value) = request.global_download_limit_mb {
            config.global_download_limit_mb = value;
        }
        if let Some(value) = request.global_upload_limit_mb {
            config.global_upload_limit_mb = value;
        }
        if let Some(value) = request.priority_decay {
            config.priority_decay = value.clamp(0.01, 1.0);
        }
        if let Some(value) = request.stall_timeout_secs {
            config.stall_timeout_secs = value.max(60);
        }
        if let Some(value) = request.total_timeout_secs {
            config.total_timeout_secs = value.max(config.stall_timeout_secs);
        }
        if let Some(value) = request.scheduler_interval_secs {
            config.scheduler_interval_secs = value.clamp(1, 30);
        }
        drop(config);

        self.runtime_overview().await
    }

    pub async fn runtime_overview(&self) -> anyhow::Result<RuntimeOverviewResponse> {
        let config = self.config.read().await.clone();
        let tasks = self.tasks.read().await;
        let plan = compute_queue_plan(&tasks, &config);
        let summary = summarize_plan(&tasks, &plan);

        Ok(RuntimeOverviewResponse {
            started_at: self.started_at,
            settings: RuntimeSettingsDto {
                default_output_dir: config.default_output_dir.to_string_lossy().into_owned(),
                max_concurrent_downloads: config.max_concurrent_downloads,
                max_concurrent_seeds: config.max_concurrent_seeds,
                global_download_limit_mb: config.global_download_limit_mb,
                global_upload_limit_mb: config.global_upload_limit_mb,
                priority_decay: config.priority_decay,
                stall_timeout_secs: config.stall_timeout_secs,
                total_timeout_secs: config.total_timeout_secs,
                scheduler_interval_secs: config.scheduler_interval_secs,
            },
            total_tasks: tasks.len(),
            enabled_tasks: tasks.values().filter(|task| task.enabled).count(),
            active_downloads: summary.active_downloads,
            active_seeds: summary.active_seeds,
            queued_downloads: summary.queued_downloads,
            queued_seeds: summary.queued_seeds,
            total_download_rate_bytes: tasks.values().map(|task| task.download_rate_bytes).sum(),
            total_upload_rate_bytes: tasks.values().map(|task| task.upload_rate_bytes).sum(),
        })
    }

    async fn scheduler_tick(&self) -> anyhow::Result<()> {
        self.refresh_runtime_stats().await?;

        let config = self.config.read().await.clone();
        let plan = {
            let tasks = self.tasks.read().await;
            compute_queue_plan(&tasks, &config)
        };

        self.enforce_timeouts().await?;
        self.apply_schedule(&plan).await?;
        Ok(())
    }

    async fn refresh_runtime_stats(&self) -> anyhow::Result<()> {
        let session_ids = self
            .sessions
            .lock()
            .await
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for task_id in session_ids {
            let snapshot = {
                let sessions = self.sessions.lock().await;
                let Some(session) = sessions.get(&task_id) else {
                    continue;
                };
                match session.stats() {
                    Ok(stats) => Some(map_torrent_stats(&stats)),
                    Err(error) => {
                        warn!(task_id = %task_id, error = %error, "Failed to read rqbit task stats");
                        None
                    }
                }
            };

            let Some(snapshot) = snapshot else {
                continue;
            };

            let mut tasks = self.tasks.write().await;
            let Some(task) = tasks.get_mut(&task_id) else {
                continue;
            };

            let now = Utc::now();
            if snapshot.downloaded_bytes > task.last_progress_bytes {
                task.last_progress_bytes = snapshot.downloaded_bytes;
                task.last_progress_at = Some(now);
            }
            if matches!(snapshot.state, TaskState::Seeding | TaskState::Completed) {
                task.completed_at.get_or_insert(now);
            }
            if matches!(snapshot.state, TaskState::Downloading | TaskState::Starting)
                && task.started_at.is_none()
            {
                task.started_at = Some(now);
            }

            task.state = if task.enabled && matches!(snapshot.state, TaskState::Paused) {
                TaskState::Queued
            } else {
                snapshot.state
            };
            task.downloaded_bytes = snapshot.downloaded_bytes;
            task.total_bytes = snapshot.total_bytes;
            task.uploaded_bytes = snapshot.uploaded_bytes;
            task.download_rate_bytes = snapshot.download_rate_bytes;
            task.upload_rate_bytes = snapshot.upload_rate_bytes;
            task.peer_count = snapshot.peer_count;
            task.updated_at = now;
        }

        Ok(())
    }

    async fn enforce_timeouts(&self) -> anyhow::Result<()> {
        let now = Utc::now();
        let mut timeouts = Vec::new();

        {
            let tasks = self.tasks.read().await;
            for task in tasks.values() {
                if !task.enabled
                    || !matches!(task.state, TaskState::Starting | TaskState::Downloading)
                {
                    continue;
                }

                if let Some(started_at) = task.started_at {
                    let total_elapsed = (now - started_at).num_seconds().max(0) as u64;
                    if total_elapsed >= task.total_timeout_secs {
                        timeouts.push((task.id, "total timeout reached".to_owned()));
                        continue;
                    }
                }

                if let Some(last_progress_at) = task.last_progress_at {
                    let stall_elapsed = (now - last_progress_at).num_seconds().max(0) as u64;
                    if stall_elapsed >= task.stall_timeout_secs {
                        timeouts.push((task.id, "stalled without download progress".to_owned()));
                    }
                }
            }
        }

        for (task_id, reason) in timeouts {
            if let Some(session) = self.sessions.lock().await.get(&task_id) {
                if let Err(error) = session.pause().await {
                    warn!(task_id = %task_id, error = %error, "Failed to pause timed-out task");
                }
            }

            let mut tasks = self.tasks.write().await;
            let Some(task) = tasks.get_mut(&task_id) else {
                continue;
            };
            task.enabled = false;
            task.state = TaskState::Failed;
            task.last_error = Some(reason);
            task.updated_at = Utc::now();
        }

        Ok(())
    }

    async fn apply_schedule(&self, plan: &QueuePlan) -> anyhow::Result<()> {
        let active_ids = plan
            .active_downloads
            .iter()
            .chain(plan.active_seeds.iter())
            .copied()
            .collect::<HashSet<_>>();

        let task_ids = self.tasks.read().await.keys().copied().collect::<Vec<_>>();
        for task_id in task_ids {
            let task = {
                let tasks = self.tasks.read().await;
                tasks.get(&task_id).cloned()
            };
            let Some(task) = task else {
                continue;
            };

            if active_ids.contains(&task_id) {
                let download_limit = plan.download_limits.get(&task_id).copied().flatten();
                let upload_limit = plan.upload_limits.get(&task_id).copied().flatten();
                self.ensure_task_active(task, download_limit, upload_limit)
                    .await?;
            } else {
                self.ensure_task_inactive(task).await?;
            }
        }

        {
            let mut tasks = self.tasks.write().await;
            for task in tasks.values_mut() {
                task.effective_download_limit_bps =
                    plan.download_limits.get(&task.id).copied().flatten();
                task.effective_upload_limit_bps =
                    plan.upload_limits.get(&task.id).copied().flatten();
            }
        }

        Ok(())
    }

    async fn ensure_task_active(
        &self,
        task: TaskRecord,
        download_limit_bps: Option<u64>,
        upload_limit_bps: Option<u64>,
    ) -> anyhow::Result<()> {
        let has_session = self.sessions.lock().await.contains_key(&task.id);
        if !has_session {
            let session = self
                .start_task_session(&task, download_limit_bps, upload_limit_bps)
                .await?;
            self.sessions.lock().await.insert(task.id, session);

            let mut tasks = self.tasks.write().await;
            if let Some(record) = tasks.get_mut(&task.id) {
                record.state = TaskState::Starting;
                record.started_at.get_or_insert(Utc::now());
                record.updated_at = Utc::now();
            }
            return Ok(());
        }

        let sessions = self.sessions.lock().await;
        let Some(session) = sessions.get(&task.id) else {
            return Ok(());
        };
        session.apply_limits(download_limit_bps, upload_limit_bps);
        if matches!(
            task.state,
            TaskState::Paused | TaskState::Queued | TaskState::Completed
        ) {
            if let Err(error) = session.resume().await {
                warn!(task_id = %task.id, error = %error, "Failed to resume active task");
                let mut tasks = self.tasks.write().await;
                if let Some(record) = tasks.get_mut(&task.id) {
                    record.state = TaskState::Failed;
                    record.enabled = false;
                    record.last_error = Some(error.to_string());
                    record.updated_at = Utc::now();
                }
                return Ok(());
            }
        }
        drop(sessions);

        let mut tasks = self.tasks.write().await;
        if let Some(record) = tasks.get_mut(&task.id) {
            if is_task_finished(record) {
                record.state = TaskState::Seeding;
            } else if !matches!(record.state, TaskState::Downloading | TaskState::Starting) {
                record.state = TaskState::Queued;
            }
            record.updated_at = Utc::now();
        }

        Ok(())
    }

    async fn ensure_task_inactive(&self, task: TaskRecord) -> anyhow::Result<()> {
        let sessions = self.sessions.lock().await;
        let Some(session) = sessions.get(&task.id) else {
            return Ok(());
        };

        if matches!(
            task.state,
            TaskState::Downloading | TaskState::Starting | TaskState::Seeding
        ) {
            if let Err(error) = session.pause().await {
                warn!(task_id = %task.id, error = %error, "Failed to pause inactive task");
            }
        }
        drop(sessions);

        let mut tasks = self.tasks.write().await;
        if let Some(record) = tasks.get_mut(&task.id) {
            record.state = if record.enabled {
                if is_task_finished(record)
                    && (record.kind == TaskKind::Seed || record.seed_after_download)
                {
                    TaskState::Completed
                } else {
                    TaskState::Queued
                }
            } else {
                TaskState::Paused
            };
            record.download_rate_bytes = 0;
            record.upload_rate_bytes = 0;
            record.updated_at = Utc::now();
        }

        Ok(())
    }

    async fn start_task_session(
        &self,
        task: &TaskRecord,
        download_limit_bps: Option<u64>,
        upload_limit_bps: Option<u64>,
    ) -> anyhow::Result<TaskSession> {
        let runtime_root = self
            .config
            .read()
            .await
            .runtime_root
            .join("tasks")
            .join(task.id.to_string());
        let output_dir = PathBuf::from(&task.output_dir);
        let session = build_session(
            &runtime_root,
            &output_dir,
            download_limit_bps,
            upload_limit_bps,
        )
        .await?;
        let api = RqbitApi::new(session.clone(), None);
        let response = api
            .api_add_torrent(
                add_torrent_from_source(&task.source)?,
                Some(AddTorrentOptions {
                    paused: false,
                    overwrite: true,
                    output_folder: Some(output_dir.to_string_lossy().into_owned()),
                    ratelimits: LimitsConfig {
                        download_bps: limit_to_non_zero(download_limit_bps),
                        upload_bps: limit_to_non_zero(upload_limit_bps),
                    },
                    ..Default::default()
                }),
            )
            .await
            .map_err(|error| anyhow!(error.to_string()))?;
        let torrent_ref = TorrentIdOrHash::Id(
            response
                .id
                .ok_or_else(|| anyhow!("rqbit did not return a torrent id"))?,
        );

        let metadata = metadata_from_add_response(response);
        {
            let mut tasks = self.tasks.write().await;
            if let Some(record) = tasks.get_mut(&task.id) {
                record.display_name = metadata.name.clone();
                record.info_hash = Some(metadata.info_hash.clone());
                record.engine_id = Some(metadata.info_hash.clone());
                record.metadata = Some(metadata);
                record.updated_at = Utc::now();
            }
        }

        info!(
            task_id = %task.id,
            kind = ?task.kind,
            priority = task.priority,
            output_dir = %task.output_dir,
            "Started standalone downloader task session"
        );

        Ok(TaskSession {
            session,
            api,
            torrent_ref,
        })
    }
}

pub fn build_router(service: Arc<DownloaderService>) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/v1/runtime", get(runtime))
        .route("/api/v1/settings", patch(update_settings))
        .route("/api/v1/inspect", post(inspect))
        .route("/api/v1/tasks", get(list_tasks).post(create_task))
        .route(
            "/api/v1/tasks/{task_id}",
            get(get_task).patch(update_task).delete(delete_task),
        )
        .route("/api/v1/tasks/{task_id}/pause", post(pause_task))
        .route("/api/v1/tasks/{task_id}/resume", post(resume_task))
        .route("/api/v1/downloads", get(list_downloads))
        .route("/api/v1/seeds", get(list_seeds))
        .with_state(service)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
}

async fn health(
    State(service): State<Arc<DownloaderService>>,
) -> Result<Json<ApiEnvelope<RuntimeOverviewResponse>>, (StatusCode, Json<ErrorPayload>)> {
    let runtime = service.runtime_overview().await.map_err(internal_error)?;
    Ok(Json(ApiEnvelope { data: runtime }))
}

async fn runtime(
    State(service): State<Arc<DownloaderService>>,
) -> Result<Json<ApiEnvelope<RuntimeOverviewResponse>>, (StatusCode, Json<ErrorPayload>)> {
    let runtime = service.runtime_overview().await.map_err(internal_error)?;
    Ok(Json(ApiEnvelope { data: runtime }))
}

async fn update_settings(
    State(service): State<Arc<DownloaderService>>,
    Json(payload): Json<UpdateSettingsRequest>,
) -> Result<Json<ApiEnvelope<RuntimeOverviewResponse>>, (StatusCode, Json<ErrorPayload>)> {
    let runtime = service
        .update_settings(payload)
        .await
        .map_err(internal_error)?;
    Ok(Json(ApiEnvelope { data: runtime }))
}

async fn inspect(
    State(service): State<Arc<DownloaderService>>,
    Json(payload): Json<InspectTaskRequest>,
) -> Result<Json<ApiEnvelope<TorrentMetadataSummary>>, (StatusCode, Json<ErrorPayload>)> {
    let metadata = service
        .inspect_source(payload)
        .await
        .map_err(invalid_error)?;
    Ok(Json(ApiEnvelope { data: metadata }))
}

async fn create_task(
    State(service): State<Arc<DownloaderService>>,
    Json(payload): Json<CreateTaskRequest>,
) -> Result<Json<ApiEnvelope<CreateTaskResponse>>, (StatusCode, Json<ErrorPayload>)> {
    let task = service.create_task(payload).await.map_err(invalid_error)?;
    Ok(Json(ApiEnvelope { data: task }))
}

async fn list_tasks(
    State(service): State<Arc<DownloaderService>>,
) -> Result<Json<ApiEnvelope<TaskListResponse>>, (StatusCode, Json<ErrorPayload>)> {
    let list = service.list_all_tasks().await;
    Ok(Json(ApiEnvelope { data: list }))
}

async fn list_downloads(
    State(service): State<Arc<DownloaderService>>,
) -> Result<Json<ApiEnvelope<TaskListResponse>>, (StatusCode, Json<ErrorPayload>)> {
    let list = service.list_downloads().await;
    Ok(Json(ApiEnvelope { data: list }))
}

async fn list_seeds(
    State(service): State<Arc<DownloaderService>>,
) -> Result<Json<ApiEnvelope<TaskListResponse>>, (StatusCode, Json<ErrorPayload>)> {
    let list = service.list_seeds().await;
    Ok(Json(ApiEnvelope { data: list }))
}

async fn get_task(
    State(service): State<Arc<DownloaderService>>,
    AxumPath(task_id): AxumPath<Uuid>,
) -> Result<Json<ApiEnvelope<DownloaderTaskDto>>, (StatusCode, Json<ErrorPayload>)> {
    let task = service
        .get_task(task_id)
        .await
        .map_err(not_found_or_invalid)?;
    Ok(Json(ApiEnvelope { data: task }))
}

async fn pause_task(
    State(service): State<Arc<DownloaderService>>,
    AxumPath(task_id): AxumPath<Uuid>,
) -> Result<Json<ApiEnvelope<DownloaderTaskDto>>, (StatusCode, Json<ErrorPayload>)> {
    let task = service
        .pause_task(task_id)
        .await
        .map_err(not_found_or_invalid)?;
    Ok(Json(ApiEnvelope { data: task }))
}

async fn resume_task(
    State(service): State<Arc<DownloaderService>>,
    AxumPath(task_id): AxumPath<Uuid>,
) -> Result<Json<ApiEnvelope<DownloaderTaskDto>>, (StatusCode, Json<ErrorPayload>)> {
    let task = service
        .resume_task(task_id)
        .await
        .map_err(not_found_or_invalid)?;
    Ok(Json(ApiEnvelope { data: task }))
}

async fn update_task(
    State(service): State<Arc<DownloaderService>>,
    AxumPath(task_id): AxumPath<Uuid>,
    Json(payload): Json<UpdateTaskRequest>,
) -> Result<Json<ApiEnvelope<DownloaderTaskDto>>, (StatusCode, Json<ErrorPayload>)> {
    let task = service
        .update_task(task_id, payload)
        .await
        .map_err(not_found_or_invalid)?;
    Ok(Json(ApiEnvelope { data: task }))
}

async fn delete_task(
    State(service): State<Arc<DownloaderService>>,
    AxumPath(task_id): AxumPath<Uuid>,
    Query(query): Query<DeleteTaskQuery>,
) -> Result<Json<ApiEnvelope<Option<DownloaderTaskDto>>>, (StatusCode, Json<ErrorPayload>)> {
    let task = service
        .delete_task(task_id, query.delete_files.unwrap_or(true))
        .await
        .map_err(not_found_or_invalid)?;
    Ok(Json(ApiEnvelope { data: task }))
}

fn internal_error(error: anyhow::Error) -> (StatusCode, Json<ErrorPayload>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorPayload {
            message: error.to_string(),
        }),
    )
}

fn invalid_error(error: anyhow::Error) -> (StatusCode, Json<ErrorPayload>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorPayload {
            message: error.to_string(),
        }),
    )
}

fn not_found_or_invalid(error: anyhow::Error) -> (StatusCode, Json<ErrorPayload>) {
    let message = error.to_string();
    let status = if message.contains("not found") {
        StatusCode::NOT_FOUND
    } else {
        StatusCode::BAD_REQUEST
    };

    (status, Json(ErrorPayload { message }))
}

fn matches_category(task: &TaskRecord, filter: Option<QueueCategory>) -> bool {
    match filter {
        None => true,
        Some(QueueCategory::Download) => task.kind == TaskKind::Download,
        Some(QueueCategory::Seed) => {
            task.kind == TaskKind::Seed
                || (task.kind == TaskKind::Download
                    && (task.seed_after_download || is_task_finished(task)))
        }
    }
}

fn summarize_plan(tasks: &HashMap<Uuid, TaskRecord>, plan: &QueuePlan) -> SchedulerSummary {
    let active_downloads = plan.active_downloads.len();
    let active_seeds = plan.active_seeds.len();
    let active = plan
        .active_downloads
        .iter()
        .chain(plan.active_seeds.iter())
        .copied()
        .collect::<HashSet<_>>();

    let queued_downloads = tasks
        .values()
        .filter(|task| matches_category(task, Some(QueueCategory::Download)))
        .filter(|task| task.enabled && !active.contains(&task.id))
        .count();
    let queued_seeds = tasks
        .values()
        .filter(|task| matches_category(task, Some(QueueCategory::Seed)))
        .filter(|task| task.enabled && !active.contains(&task.id))
        .count();

    SchedulerSummary {
        active_downloads,
        active_seeds,
        queued_downloads,
        queued_seeds,
    }
}

fn compute_queue_plan(tasks: &HashMap<Uuid, TaskRecord>, config: &DownloaderConfig) -> QueuePlan {
    let mut download_candidates = tasks
        .values()
        .filter(|task| is_download_candidate(task))
        .cloned()
        .collect::<Vec<_>>();
    download_candidates.sort_by(download_ordering);

    let mut seed_candidates = tasks
        .values()
        .filter(|task| is_seed_candidate(task))
        .cloned()
        .collect::<Vec<_>>();
    seed_candidates.sort_by(seed_ordering);

    let active_downloads = download_candidates
        .iter()
        .take(config.max_concurrent_downloads)
        .map(|task| task.id)
        .collect::<Vec<_>>();
    let active_seeds = seed_candidates
        .iter()
        .take(config.max_concurrent_seeds)
        .map(|task| task.id)
        .collect::<Vec<_>>();

    let mut queue_positions = HashMap::new();
    for (index, task) in download_candidates.iter().enumerate() {
        queue_positions.insert(task.id, index + 1);
    }
    for (index, task) in seed_candidates.iter().enumerate() {
        queue_positions.entry(task.id).or_insert(index + 1);
    }

    let download_limits = compute_download_limits(
        tasks,
        &active_downloads,
        config.global_download_limit_mb,
        config.priority_decay,
    );
    let upload_limits =
        compute_seed_upload_limits(tasks, &active_seeds, config.global_upload_limit_mb);

    QueuePlan {
        active_downloads,
        active_seeds,
        queue_positions,
        download_limits,
        upload_limits,
    }
}

fn compute_download_limits(
    tasks: &HashMap<Uuid, TaskRecord>,
    active_ids: &[Uuid],
    total_limit_mb: u64,
    decay: f64,
) -> HashMap<Uuid, Option<u64>> {
    let mut limits = HashMap::new();
    if active_ids.is_empty() {
        return limits;
    }

    let total_limit_bps = mb_to_bytes_per_second(total_limit_mb);
    let mut remaining = total_limit_bps.unwrap_or(0);
    let mut auto_layers = BTreeMap::<u32, Vec<Uuid>>::new();

    for task_id in active_ids {
        let Some(task) = tasks.get(task_id) else {
            continue;
        };
        if let Some(manual_mb) = task.manual_download_limit_mb {
            let manual_bps = mb_to_bytes_per_second(manual_mb).unwrap_or(0);
            limits.insert(*task_id, Some(manual_bps));
            if total_limit_bps.is_some() {
                remaining = remaining.saturating_sub(manual_bps);
            }
        } else {
            auto_layers.entry(task.priority).or_default().push(*task_id);
        }
    }

    if total_limit_bps.is_none() {
        for task_ids in auto_layers.values() {
            for task_id in task_ids {
                limits.insert(*task_id, None);
            }
        }
        return limits;
    }

    if remaining == 0 {
        for task_ids in auto_layers.values() {
            for task_id in task_ids {
                limits.insert(*task_id, Some(0));
            }
        }
        return limits;
    }

    let mut weight_sum = 0.0_f64;
    for (layer_index, (_, task_ids)) in auto_layers.iter().enumerate() {
        weight_sum += decay.powi(layer_index as i32) * task_ids.len() as f64;
    }

    if weight_sum <= f64::EPSILON {
        return limits;
    }

    let base = remaining as f64 / weight_sum;
    for (layer_index, (_, task_ids)) in auto_layers.iter().enumerate() {
        let layer_limit = (base * decay.powi(layer_index as i32)).round().max(0.0) as u64;
        for task_id in task_ids {
            limits.insert(*task_id, Some(layer_limit));
        }
    }

    limits
}

fn compute_seed_upload_limits(
    tasks: &HashMap<Uuid, TaskRecord>,
    active_ids: &[Uuid],
    total_limit_mb: u64,
) -> HashMap<Uuid, Option<u64>> {
    let mut limits = HashMap::new();
    if active_ids.is_empty() {
        return limits;
    }

    let total_limit_bps = mb_to_bytes_per_second(total_limit_mb);
    let mut remaining = total_limit_bps.unwrap_or(0);
    let mut auto_ids = Vec::new();

    for task_id in active_ids {
        let Some(task) = tasks.get(task_id) else {
            continue;
        };
        if let Some(manual_mb) = task.manual_upload_limit_mb {
            let manual_bps = mb_to_bytes_per_second(manual_mb).unwrap_or(0);
            limits.insert(*task_id, Some(manual_bps));
            if total_limit_bps.is_some() {
                remaining = remaining.saturating_sub(manual_bps);
            }
        } else {
            auto_ids.push(*task_id);
        }
    }

    if total_limit_bps.is_none() {
        for task_id in auto_ids {
            limits.insert(task_id, None);
        }
        return limits;
    }

    let divisor = auto_ids.len().max(1) as u64;
    let share = remaining / divisor;
    for task_id in auto_ids {
        limits.insert(task_id, Some(share));
    }

    limits
}

fn is_download_candidate(task: &TaskRecord) -> bool {
    if !task.enabled {
        return false;
    }
    if matches!(task.state, TaskState::Failed | TaskState::Deleted) {
        return false;
    }
    match task.kind {
        TaskKind::Download => !is_task_finished(task),
        TaskKind::Seed => false,
    }
}

fn is_seed_candidate(task: &TaskRecord) -> bool {
    if !task.enabled {
        return false;
    }
    if matches!(task.state, TaskState::Failed | TaskState::Deleted) {
        return false;
    }

    match task.kind {
        TaskKind::Seed => true,
        TaskKind::Download => task.seed_after_download && is_task_finished(task),
    }
}

fn is_task_finished(task: &TaskRecord) -> bool {
    matches!(task.state, TaskState::Seeding | TaskState::Completed)
        || (task.total_bytes > 0 && task.downloaded_bytes >= task.total_bytes)
}

fn download_ordering(left: &TaskRecord, right: &TaskRecord) -> std::cmp::Ordering {
    left.priority
        .cmp(&right.priority)
        .then_with(|| left.created_at.cmp(&right.created_at))
        .then_with(|| left.id.cmp(&right.id))
}

fn seed_ordering(left: &TaskRecord, right: &TaskRecord) -> std::cmp::Ordering {
    left.completed_at
        .unwrap_or(left.created_at)
        .cmp(&right.completed_at.unwrap_or(right.created_at))
        .then_with(|| left.created_at.cmp(&right.created_at))
        .then_with(|| left.id.cmp(&right.id))
}

fn map_torrent_stats(stats: &TorrentStats) -> TaskRuntimeSnapshot {
    let state = match stats.state {
        TorrentStatsState::Initializing => TaskState::Starting,
        TorrentStatsState::Live => {
            if stats.finished {
                TaskState::Seeding
            } else {
                TaskState::Downloading
            }
        }
        TorrentStatsState::Paused => {
            if stats.finished {
                TaskState::Completed
            } else {
                TaskState::Paused
            }
        }
        TorrentStatsState::Error => TaskState::Failed,
    };

    TaskRuntimeSnapshot {
        state,
        downloaded_bytes: stats.progress_bytes,
        total_bytes: stats.total_bytes,
        uploaded_bytes: stats.uploaded_bytes,
        download_rate_bytes: stats
            .live
            .as_ref()
            .map(|live| speed_mib_to_bytes(live.download_speed.mbps))
            .unwrap_or(0),
        upload_rate_bytes: stats
            .live
            .as_ref()
            .map(|live| speed_mib_to_bytes(live.upload_speed.mbps))
            .unwrap_or(0),
        peer_count: stats
            .live
            .as_ref()
            .map(|live| live.snapshot.peer_stats.live as u32)
            .unwrap_or(0),
    }
}

async fn build_session(
    runtime_root: &Path,
    output_dir: &Path,
    download_limit_bps: Option<u64>,
    upload_limit_bps: Option<u64>,
) -> anyhow::Result<Arc<Session>> {
    let session_dir = runtime_root.join("session");
    fs::create_dir_all(&session_dir).with_context(|| {
        format!(
            "failed to create downloader session root {}",
            session_dir.display()
        )
    })?;
    fs::create_dir_all(output_dir).with_context(|| {
        format!(
            "failed to create downloader output directory {}",
            output_dir.display()
        )
    })?;

    Session::new_with_opts(
        output_dir.to_path_buf(),
        SessionOptions {
            disable_dht: cfg!(windows),
            disable_dht_persistence: true,
            persistence: Some(SessionPersistenceConfig::Json {
                folder: Some(session_dir),
            }),
            ratelimits: LimitsConfig {
                download_bps: limit_to_non_zero(download_limit_bps),
                upload_bps: limit_to_non_zero(upload_limit_bps),
            },
            ..Default::default()
        },
    )
    .await
    .context("failed to initialize rqbit session")
}

fn add_torrent_from_source(source: &TaskSource) -> anyhow::Result<AddTorrent<'static>> {
    match source.kind {
        TaskSourceKind::Url => Ok(AddTorrent::from_url(source.value.clone())),
        TaskSourceKind::TorrentFile => {
            let bytes = fs::read(&source.value)
                .with_context(|| format!("failed to read torrent file {}", source.value))?;
            Ok(AddTorrent::from_bytes(bytes))
        }
    }
}

fn metadata_from_add_response(response: ApiAddTorrentResponse) -> TorrentMetadataSummary {
    let files = response
        .details
        .files
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .map(|(index, file)| TorrentFileEntry {
            index,
            name: file.name,
            components: file.components,
            length: file.length,
            included: file.included,
        })
        .collect::<Vec<_>>();
    let total_bytes = files.iter().map(|file| file.length).sum();

    TorrentMetadataSummary {
        info_hash: response.details.info_hash,
        name: response.details.name,
        output_folder: response.output_folder,
        total_bytes,
        file_count: files.len(),
        files,
        seen_peers: response
            .seen_peers
            .unwrap_or_default()
            .into_iter()
            .map(|value| value.to_string())
            .collect(),
    }
}

fn limit_to_non_zero(value: Option<u64>) -> Option<NonZeroU32> {
    let value = value?;
    let bounded = value.min(u32::MAX as u64) as u32;
    NonZeroU32::new(bounded)
}

fn mb_to_bytes_per_second(value: u64) -> Option<u64> {
    if value == 0 {
        None
    } else {
        Some(value.saturating_mul(1024 * 1024))
    }
}

fn bytes_per_second_to_mb_per_second(value: u64) -> f64 {
    value as f64 / 1024.0 / 1024.0
}

fn speed_mib_to_bytes(value: f64) -> u64 {
    if !value.is_finite() || value <= 0.0 {
        0
    } else {
        (value * 1024.0 * 1024.0).round() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn sample_task(
        priority: u32,
        created_at: DateTime<Utc>,
        manual_download_limit_mb: Option<u64>,
    ) -> TaskRecord {
        TaskRecord {
            id: Uuid::new_v4(),
            kind: TaskKind::Download,
            state: TaskState::Queued,
            enabled: true,
            priority,
            seed_after_download: true,
            source: TaskSource {
                kind: TaskSourceKind::Url,
                value: "magnet:?xt=urn:btih:test".to_owned(),
            },
            output_dir: "runtime/downloader/tests".to_owned(),
            display_name: None,
            info_hash: None,
            metadata: None,
            engine_id: None,
            downloaded_bytes: 0,
            total_bytes: 0,
            uploaded_bytes: 0,
            download_rate_bytes: 0,
            upload_rate_bytes: 0,
            peer_count: 0,
            manual_download_limit_mb,
            manual_upload_limit_mb: None,
            effective_download_limit_bps: None,
            effective_upload_limit_bps: None,
            stall_timeout_secs: 600,
            total_timeout_secs: 14_400,
            started_at: None,
            completed_at: None,
            last_progress_at: None,
            last_progress_bytes: 0,
            last_error: None,
            created_at,
            updated_at: created_at,
        }
    }

    #[test]
    fn download_limits_respect_priority_layers_and_manual_caps() {
        let created = Utc
            .with_ymd_and_hms(2026, 3, 27, 0, 0, 0)
            .single()
            .expect("valid timestamp");

        let tasks = vec![
            sample_task(3, created, None),
            sample_task(7, created + chrono::Duration::seconds(1), None),
            sample_task(7, created + chrono::Duration::seconds(2), None),
            sample_task(4, created + chrono::Duration::seconds(3), None),
            sample_task(6, created + chrono::Duration::seconds(4), Some(2)),
        ];

        let active_ids = tasks.iter().map(|task| task.id).collect::<Vec<_>>();
        let task_map = tasks
            .into_iter()
            .map(|task| (task.id, task))
            .collect::<HashMap<_, _>>();

        let limits = compute_download_limits(&task_map, &active_ids, 5, 0.8);

        let manual_task = task_map
            .values()
            .find(|task| task.priority == 6)
            .expect("manual task exists");
        let p3_task = task_map
            .values()
            .find(|task| task.priority == 3)
            .expect("priority 3 task exists");
        let p4_task = task_map
            .values()
            .find(|task| task.priority == 4)
            .expect("priority 4 task exists");
        let p7_tasks = task_map
            .values()
            .filter(|task| task.priority == 7)
            .collect::<Vec<_>>();

        assert_eq!(
            limits.get(&manual_task.id).copied().flatten(),
            Some(2 * 1024 * 1024)
        );

        let p3 = limits
            .get(&p3_task.id)
            .copied()
            .flatten()
            .expect("p3 limit");
        let p4 = limits
            .get(&p4_task.id)
            .copied()
            .flatten()
            .expect("p4 limit");
        let p7 = limits
            .get(&p7_tasks[0].id)
            .copied()
            .flatten()
            .expect("p7 limit");
        let p7_second = limits
            .get(&p7_tasks[1].id)
            .copied()
            .flatten()
            .expect("p7 second limit");

        assert!(p3 > p4);
        assert!(p4 > p7);
        assert_eq!(p7, p7_second);

        let auto_total = p3 + p4 + p7 + p7_second;
        let expected_remaining = 3 * 1024 * 1024;
        assert!((auto_total as i64 - expected_remaining as i64).abs() <= 8);
    }

    #[test]
    fn queue_plan_prefers_lower_priority_value_then_creation_time() {
        let created = Utc
            .with_ymd_and_hms(2026, 3, 27, 1, 0, 0)
            .single()
            .expect("valid timestamp");

        let first = sample_task(0, created, None);
        let second = sample_task(5, created + chrono::Duration::seconds(1), None);
        let third = sample_task(5, created + chrono::Duration::seconds(2), None);

        let first_id = first.id;
        let second_id = second.id;
        let third_id = third.id;

        let tasks = [first, second, third]
            .into_iter()
            .map(|task| (task.id, task))
            .collect::<HashMap<_, _>>();

        let config = DownloaderConfig {
            max_concurrent_downloads: 1,
            ..DownloaderConfig::default()
        };

        let plan = compute_queue_plan(&tasks, &config);

        assert_eq!(plan.active_downloads, vec![first_id]);
        assert_eq!(plan.queue_positions.get(&first_id), Some(&1));
        assert_eq!(plan.queue_positions.get(&second_id), Some(&2));
        assert_eq!(plan.queue_positions.get(&third_id), Some(&3));
    }
}
