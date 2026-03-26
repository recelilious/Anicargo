use std::{
    fs,
    num::NonZeroU32,
    path::Path,
    sync::{Arc, RwLock},
};

use anicargo_downloader::{
    DownloaderService,
    model::{
        CreateTaskRequest as EmbeddedCreateTaskRequest, TaskKind as EmbeddedTaskKind,
        InspectTaskRequest as EmbeddedInspectTaskRequest,
        TaskSource as EmbeddedTaskSource, TaskSourceKind as EmbeddedTaskSourceKind,
        TaskState as EmbeddedTaskState, UpdateSettingsRequest as EmbeddedUpdateSettingsRequest,
    },
};
use anyhow::{Context, anyhow};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use librqbit::api::TorrentIdOrHash;
use librqbit::{
    AddTorrent, AddTorrentOptions, Session, SessionOptions, SessionPersistenceConfig, TorrentStats,
    TorrentStatsState,
};
use sqlx::SqlitePool;
use tracing::{info, warn};
use uuid::Uuid;

use crate::{
    db,
    media::{ParsedReleaseSlot, scan_video_files},
    types::{
        AppError, DownloadDecisionDto, DownloadExecutionDecisionDto, DownloadExecutionDto,
        DownloadJobDto, ResourceCandidateDto,
    },
};

#[derive(Debug, Clone)]
pub struct DownloadDemandInput {
    pub bangumi_subject_id: i64,
    pub release_status: String,
    pub subscription_count: i64,
    pub threshold: i64,
    pub trigger_kind: &'static str,
    pub requested_by: String,
    pub force: bool,
}

#[derive(Debug, Clone)]
pub struct EngineQueueRequest {
    pub bangumi_subject_id: i64,
    pub release_status: String,
    pub season_mode: String,
    pub trigger_kind: String,
    pub requested_by: String,
    pub subscription_count: i64,
    pub threshold_snapshot: i64,
}

#[derive(Debug, Clone)]
pub struct EngineQueueAccepted {
    pub lifecycle: String,
    pub engine_job_ref: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EngineActivateRequest {
    pub download_job_id: i64,
    pub bangumi_subject_id: i64,
    pub resource_candidate_id: i64,
    pub priority: u32,
    pub provider: String,
    pub provider_resource_id: String,
    pub title: String,
    pub magnet: String,
    pub size_bytes: i64,
    pub fansub_name: Option<String>,
    pub target_path: String,
    pub execution_role: String,
}

#[derive(Debug, Clone)]
pub struct EngineActivateAccepted {
    pub state: String,
    pub engine_execution_ref: Option<String>,
    pub notes: Option<String>,
    pub downloaded_bytes: i64,
    pub total_bytes: i64,
    pub uploaded_bytes: i64,
    pub download_rate_bytes: i64,
    pub upload_rate_bytes: i64,
    pub peer_count: i64,
}

#[derive(Debug, Clone)]
pub struct EngineSyncAccepted {
    pub state: String,
    pub notes: Option<String>,
    pub downloaded_bytes: i64,
    pub total_bytes: i64,
    pub uploaded_bytes: i64,
    pub download_rate_bytes: i64,
    pub upload_rate_bytes: i64,
    pub peer_count: i64,
}

#[derive(Debug, Clone)]
pub struct EngineProbeAccepted {
    pub peer_count: Option<i64>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct DownloadRuntimeSettings {
    pub max_concurrent_downloads: usize,
    pub upload_limit_mb: u64,
    pub download_limit_mb: u64,
}

impl DownloadRuntimeSettings {
    pub fn new(
        max_concurrent_downloads: usize,
        upload_limit_mb: u64,
        download_limit_mb: u64,
    ) -> Self {
        Self {
            max_concurrent_downloads: max_concurrent_downloads.max(1),
            upload_limit_mb,
            download_limit_mb,
        }
    }
}

#[async_trait]
pub trait DownloadEngine: Send + Sync {
    fn name(&self) -> &'static str;
    async fn apply_runtime_settings(&self, settings: DownloadRuntimeSettings)
    -> anyhow::Result<()>;
    async fn queue(&self, request: EngineQueueRequest) -> anyhow::Result<EngineQueueAccepted>;
    async fn probe(&self, _request: &EngineActivateRequest) -> anyhow::Result<EngineProbeAccepted> {
        Ok(EngineProbeAccepted {
            peer_count: None,
            notes: None,
        })
    }
    async fn activate(
        &self,
        request: EngineActivateRequest,
    ) -> anyhow::Result<EngineActivateAccepted>;
    async fn sync_execution(
        &self,
        execution: &DownloadExecutionDto,
    ) -> anyhow::Result<EngineSyncAccepted>;
    async fn deactivate(
        &self,
        execution: &DownloadExecutionDto,
        delete_files: bool,
    ) -> anyhow::Result<()>;
}

#[derive(Debug, Default)]
pub struct PlanningDownloadEngine;

#[async_trait]
impl DownloadEngine for PlanningDownloadEngine {
    fn name(&self) -> &'static str {
        "planning"
    }

    async fn apply_runtime_settings(
        &self,
        _settings: DownloadRuntimeSettings,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn queue(&self, request: EngineQueueRequest) -> anyhow::Result<EngineQueueAccepted> {
        info!(
            subject_id = request.bangumi_subject_id,
            release_status = %request.release_status,
            season_mode = %request.season_mode,
            trigger_kind = %request.trigger_kind,
            requested_by = %request.requested_by,
            subscription_count = request.subscription_count,
            threshold = request.threshold_snapshot,
            "Download request accepted by planning engine"
        );

        Ok(EngineQueueAccepted {
            lifecycle: "queued".to_owned(),
            engine_job_ref: None,
            notes: Some(
                "Queued in planning engine; torrent execution will be attached in the next stage"
                    .to_owned(),
            ),
        })
    }

    async fn activate(
        &self,
        request: EngineActivateRequest,
    ) -> anyhow::Result<EngineActivateAccepted> {
        info!(
            job_id = request.download_job_id,
            subject_id = request.bangumi_subject_id,
            candidate_id = request.resource_candidate_id,
            provider = %request.provider,
            provider_resource_id = %request.provider_resource_id,
            title = %request.title,
            size_bytes = request.size_bytes,
            fansub = ?request.fansub_name,
            magnet_length = request.magnet.len(),
            execution_role = %request.execution_role,
            target_path = %request.target_path,
            "Selected resource staged for execution by planning engine"
        );

        Ok(EngineActivateAccepted {
            state: "staged".to_owned(),
            engine_execution_ref: None,
            notes: Some(
                "Selected resource staged for the future embedded torrent engine".to_owned(),
            ),
            downloaded_bytes: 0,
            total_bytes: request.size_bytes.max(0),
            uploaded_bytes: 0,
            download_rate_bytes: 0,
            upload_rate_bytes: 0,
            peer_count: 0,
        })
    }

    async fn sync_execution(
        &self,
        execution: &DownloadExecutionDto,
    ) -> anyhow::Result<EngineSyncAccepted> {
        Ok(EngineSyncAccepted {
            state: execution.state.clone(),
            notes: execution.notes.clone(),
            downloaded_bytes: execution.downloaded_bytes,
            total_bytes: execution.source_size_bytes.max(execution.downloaded_bytes),
            uploaded_bytes: execution.uploaded_bytes,
            download_rate_bytes: execution.download_rate_bytes,
            upload_rate_bytes: execution.upload_rate_bytes,
            peer_count: execution.peer_count,
        })
    }

    async fn deactivate(
        &self,
        execution: &DownloadExecutionDto,
        _delete_files: bool,
    ) -> anyhow::Result<()> {
        info!(
            execution_id = execution.id,
            state = %execution.state,
            "Planning engine received deactivate request"
        );
        Ok(())
    }
}

#[derive(Clone)]
pub struct EmbeddedDownloaderEngine {
    service: Arc<DownloaderService>,
}

impl EmbeddedDownloaderEngine {
    pub fn new(service: Arc<DownloaderService>) -> Self {
        Self { service }
    }

    fn parse_execution_ref(execution_ref: &str) -> anyhow::Result<Uuid> {
        Uuid::parse_str(execution_ref)
            .with_context(|| format!("invalid downloader task ref '{execution_ref}'"))
    }
}

#[async_trait]
impl DownloadEngine for EmbeddedDownloaderEngine {
    fn name(&self) -> &'static str {
        "downloader"
    }

    async fn apply_runtime_settings(
        &self,
        settings: DownloadRuntimeSettings,
    ) -> anyhow::Result<()> {
        self.service
            .update_settings(EmbeddedUpdateSettingsRequest {
                default_output_dir: None,
                max_concurrent_downloads: Some(settings.max_concurrent_downloads),
                max_concurrent_seeds: None,
                global_download_limit_mb: Some(settings.download_limit_mb),
                global_upload_limit_mb: Some(settings.upload_limit_mb),
                priority_decay: None,
                stall_timeout_secs: None,
                total_timeout_secs: None,
                scheduler_interval_secs: None,
            })
            .await
            .context("failed to apply embedded downloader settings")?;

        info!(
            max_concurrent_downloads = settings.max_concurrent_downloads,
            upload_limit_mb = settings.upload_limit_mb,
            download_limit_mb = settings.download_limit_mb,
            "Applied runtime download settings to embedded downloader engine"
        );

        Ok(())
    }

    async fn queue(&self, request: EngineQueueRequest) -> anyhow::Result<EngineQueueAccepted> {
        info!(
            subject_id = request.bangumi_subject_id,
            release_status = %request.release_status,
            season_mode = %request.season_mode,
            trigger_kind = %request.trigger_kind,
            requested_by = %request.requested_by,
            subscription_count = request.subscription_count,
            threshold = request.threshold_snapshot,
            "Download request accepted by embedded downloader engine"
        );

        Ok(EngineQueueAccepted {
            lifecycle: "queued".to_owned(),
            engine_job_ref: None,
            notes: Some("Queued for embedded downloader task creation".to_owned()),
        })
    }

    async fn probe(&self, request: &EngineActivateRequest) -> anyhow::Result<EngineProbeAccepted> {
        let metadata = self
            .service
            .inspect_source(EmbeddedInspectTaskRequest {
                source: EmbeddedTaskSource {
                    kind: EmbeddedTaskSourceKind::Url,
                    value: request.magnet.clone(),
                },
                output_dir: Some(request.target_path.clone()),
                force_network_probe: Some(true),
            })
            .await
            .with_context(|| {
                format!(
                    "failed to inspect embedded downloader source for subject {} candidate {}",
                    request.bangumi_subject_id, request.resource_candidate_id
                )
            })?;

        Ok(EngineProbeAccepted {
            peer_count: Some(metadata.seen_peers.len() as i64),
            notes: if metadata.seen_peers.is_empty() {
                Some("Source inspection found no reachable peers".to_owned())
            } else {
                Some(format!(
                    "Source inspection found {} reachable peers",
                    metadata.seen_peers.len()
                ))
            },
        })
    }

    async fn activate(
        &self,
        request: EngineActivateRequest,
    ) -> anyhow::Result<EngineActivateAccepted> {
        let created = self
            .service
            .create_task(EmbeddedCreateTaskRequest {
                kind: EmbeddedTaskKind::Download,
                source: EmbeddedTaskSource {
                    kind: EmbeddedTaskSourceKind::Url,
                    value: request.magnet.clone(),
                },
                output_dir: Some(request.target_path.clone()),
                priority: Some(request.priority),
                start_enabled: Some(true),
                seed_after_download: Some(true),
                manual_download_limit_mb: None,
                manual_upload_limit_mb: None,
                stall_timeout_secs: None,
                total_timeout_secs: None,
            })
            .await
            .with_context(|| {
                format!(
                    "failed to create embedded downloader task for subject {} candidate {}",
                    request.bangumi_subject_id, request.resource_candidate_id
                )
            })?;

        let task = created.task;
        let state = map_embedded_task_state(&task.state);
        let notes = Some(if created.created {
            "Created embedded downloader task".to_owned()
        } else {
            "Reused existing embedded downloader task".to_owned()
        });

        info!(
            job_id = request.download_job_id,
            subject_id = request.bangumi_subject_id,
            candidate_id = request.resource_candidate_id,
            priority = request.priority,
            state = %state,
            task_id = %task.id,
            created = created.created,
            "Selected resource activated on embedded downloader engine"
        );

        Ok(EngineActivateAccepted {
            state,
            engine_execution_ref: Some(task.id.to_string()),
            notes,
            downloaded_bytes: saturating_u64_to_i64(task.downloaded_bytes),
            total_bytes: saturating_u64_to_i64(task.total_bytes),
            uploaded_bytes: saturating_u64_to_i64(task.uploaded_bytes),
            download_rate_bytes: saturating_u64_to_i64(task.download_rate_bytes),
            upload_rate_bytes: saturating_u64_to_i64(task.upload_rate_bytes),
            peer_count: i64::from(task.peer_count),
        })
    }

    async fn sync_execution(
        &self,
        execution: &DownloadExecutionDto,
    ) -> anyhow::Result<EngineSyncAccepted> {
        let execution_ref = execution
            .engine_execution_ref
            .as_deref()
            .ok_or_else(|| anyhow!("execution {} is missing downloader task ref", execution.id))?;
        let task_id = Self::parse_execution_ref(execution_ref)?;
        let task = self.service.get_task(task_id).await.with_context(|| {
            format!(
                "failed to read embedded downloader task {} for execution {}",
                execution_ref, execution.id
            )
        })?;

        Ok(EngineSyncAccepted {
            state: map_embedded_task_state(&task.state),
            notes: task.last_error.clone(),
            downloaded_bytes: saturating_u64_to_i64(task.downloaded_bytes),
            total_bytes: saturating_u64_to_i64(task.total_bytes),
            uploaded_bytes: saturating_u64_to_i64(task.uploaded_bytes),
            download_rate_bytes: saturating_u64_to_i64(task.download_rate_bytes),
            upload_rate_bytes: saturating_u64_to_i64(task.upload_rate_bytes),
            peer_count: i64::from(task.peer_count),
        })
    }

    async fn deactivate(
        &self,
        execution: &DownloadExecutionDto,
        delete_files: bool,
    ) -> anyhow::Result<()> {
        let Some(execution_ref) = execution.engine_execution_ref.as_deref() else {
            return Ok(());
        };
        let task_id = Self::parse_execution_ref(execution_ref)?;
        self.service
            .delete_task(task_id, delete_files)
            .await
            .with_context(|| {
                format!(
                    "failed to delete embedded downloader task {} for execution {}",
                    execution_ref, execution.id
                )
            })?;
        Ok(())
    }
}

#[derive(Clone)]
pub struct RqbitDownloadEngine {
    session: Arc<Session>,
}

impl RqbitDownloadEngine {
    pub async fn new(media_root: &Path) -> anyhow::Result<Self> {
        let rqbit_root = media_root.join("_rqbit");
        let default_output_root = rqbit_root.join("downloads");
        let persistence_root = rqbit_root.join("session");

        fs::create_dir_all(&default_output_root).with_context(|| {
            format!(
                "failed to create rqbit download root {}",
                default_output_root.display()
            )
        })?;
        fs::create_dir_all(&persistence_root).with_context(|| {
            format!(
                "failed to create rqbit persistence root {}",
                persistence_root.display()
            )
        })?;

        let session = Session::new_with_opts(
            default_output_root,
            SessionOptions {
                fastresume: true,
                persistence: Some(SessionPersistenceConfig::Json {
                    folder: Some(persistence_root),
                }),
                ..Default::default()
            },
        )
        .await
        .context("failed to initialize rqbit session")?;

        Ok(Self { session })
    }

    fn parse_execution_ref(execution_ref: &str) -> anyhow::Result<TorrentIdOrHash> {
        TorrentIdOrHash::parse(execution_ref)
            .with_context(|| format!("invalid rqbit execution ref '{execution_ref}'"))
    }
}

#[async_trait]
impl DownloadEngine for RqbitDownloadEngine {
    fn name(&self) -> &'static str {
        "rqbit"
    }

    async fn apply_runtime_settings(
        &self,
        settings: DownloadRuntimeSettings,
    ) -> anyhow::Result<()> {
        self.session
            .ratelimits
            .set_upload_bps(limit_mb_to_non_zero_bps(settings.upload_limit_mb));
        self.session
            .ratelimits
            .set_download_bps(limit_mb_to_non_zero_bps(settings.download_limit_mb));

        info!(
            max_concurrent_downloads = settings.max_concurrent_downloads,
            upload_limit_mb = settings.upload_limit_mb,
            download_limit_mb = settings.download_limit_mb,
            "Applied runtime download settings to rqbit engine"
        );

        Ok(())
    }

    async fn queue(&self, request: EngineQueueRequest) -> anyhow::Result<EngineQueueAccepted> {
        info!(
            subject_id = request.bangumi_subject_id,
            release_status = %request.release_status,
            season_mode = %request.season_mode,
            trigger_kind = %request.trigger_kind,
            requested_by = %request.requested_by,
            subscription_count = request.subscription_count,
            threshold = request.threshold_snapshot,
            "Download request accepted by rqbit engine"
        );

        Ok(EngineQueueAccepted {
            lifecycle: "queued".to_owned(),
            engine_job_ref: None,
            notes: Some("Queued for embedded rqbit execution".to_owned()),
        })
    }

    async fn activate(
        &self,
        request: EngineActivateRequest,
    ) -> anyhow::Result<EngineActivateAccepted> {
        let response = self
            .session
            .add_torrent(
                AddTorrent::from_url(request.magnet.clone()),
                Some(AddTorrentOptions {
                    overwrite: true,
                    output_folder: Some(request.target_path.clone()),
                    ..Default::default()
                }),
            )
            .await
            .with_context(|| {
                format!(
                    "failed to add torrent for subject {} candidate {}",
                    request.bangumi_subject_id, request.resource_candidate_id
                )
            })?;

        let handle = response.into_handle().ok_or_else(|| {
            anyhow!(
                "rqbit did not return a torrent handle for candidate {}",
                request.resource_candidate_id
            )
        })?;
        let stats = handle.stats();
        let state = map_rqbit_state(&stats);
        let notes = rqbit_notes(&stats);
        let engine_execution_ref = Some(handle.info_hash().as_string());

        info!(
            job_id = request.download_job_id,
            subject_id = request.bangumi_subject_id,
            candidate_id = request.resource_candidate_id,
            state = %state,
            execution_ref = ?engine_execution_ref,
            "Selected resource activated on rqbit engine"
        );

        Ok(EngineActivateAccepted {
            state,
            engine_execution_ref,
            notes,
            downloaded_bytes: saturating_u64_to_i64(stats.progress_bytes),
            total_bytes: saturating_u64_to_i64(stats.total_bytes),
            uploaded_bytes: saturating_u64_to_i64(stats.uploaded_bytes),
            download_rate_bytes: rqbit_download_rate_bytes(&stats),
            upload_rate_bytes: rqbit_upload_rate_bytes(&stats),
            peer_count: rqbit_peer_count(&stats),
        })
    }

    async fn sync_execution(
        &self,
        execution: &DownloadExecutionDto,
    ) -> anyhow::Result<EngineSyncAccepted> {
        let execution_ref = execution
            .engine_execution_ref
            .as_deref()
            .ok_or_else(|| anyhow!("execution {} is missing rqbit execution ref", execution.id))?;
        let parsed_ref = Self::parse_execution_ref(execution_ref)?;
        let handle = self.session.get(parsed_ref).ok_or_else(|| {
            anyhow!(
                "rqbit execution {} is not managed by the current session",
                execution_ref
            )
        })?;
        let stats = handle.stats();

        Ok(EngineSyncAccepted {
            state: map_rqbit_state(&stats),
            notes: rqbit_notes(&stats),
            downloaded_bytes: saturating_u64_to_i64(stats.progress_bytes),
            total_bytes: saturating_u64_to_i64(stats.total_bytes),
            uploaded_bytes: saturating_u64_to_i64(stats.uploaded_bytes),
            download_rate_bytes: rqbit_download_rate_bytes(&stats),
            upload_rate_bytes: rqbit_upload_rate_bytes(&stats),
            peer_count: rqbit_peer_count(&stats),
        })
    }

    async fn deactivate(
        &self,
        execution: &DownloadExecutionDto,
        delete_files: bool,
    ) -> anyhow::Result<()> {
        let Some(execution_ref) = execution.engine_execution_ref.as_deref() else {
            return Ok(());
        };
        let parsed_ref = Self::parse_execution_ref(execution_ref)?;

        self.session
            .delete(parsed_ref, delete_files)
            .await
            .with_context(|| {
                format!(
                    "failed to delete rqbit execution {} for download execution {}",
                    execution_ref, execution.id
                )
            })?;

        Ok(())
    }
}

#[derive(Clone)]
pub struct DownloadCoordinator {
    engine: Arc<dyn DownloadEngine>,
    runtime_settings: Arc<RwLock<DownloadRuntimeSettings>>,
}

impl DownloadCoordinator {
    pub fn new(engine: Arc<dyn DownloadEngine>, runtime_settings: DownloadRuntimeSettings) -> Self {
        Self {
            engine,
            runtime_settings: Arc::new(RwLock::new(runtime_settings)),
        }
    }

    pub fn engine_name(&self) -> &'static str {
        self.engine.name()
    }

    pub async fn apply_runtime_settings(
        &self,
        settings: DownloadRuntimeSettings,
    ) -> Result<(), AppError> {
        self.engine
            .apply_runtime_settings(settings)
            .await
            .map_err(|error| {
                warn!(
                    engine = self.engine.name(),
                    error = %error,
                    "Failed to apply runtime download settings"
                );
                AppError::internal("failed to apply runtime download settings")
            })?;

        *self
            .runtime_settings
            .write()
            .expect("download runtime settings lock poisoned") = settings;

        Ok(())
    }

    pub async fn reconcile_subscription_demand(
        &self,
        pool: &SqlitePool,
        input: DownloadDemandInput,
    ) -> Result<DownloadDecisionDto, AppError> {
        let demand_state =
            determine_demand_state(input.subscription_count, input.threshold, input.force);

        db::upsert_download_subject(
            pool,
            input.bangumi_subject_id,
            &input.release_status,
            demand_state,
            input.subscription_count,
            input.threshold,
        )
        .await?;

        if !should_queue_job(input.subscription_count, input.threshold, input.force) {
            return Ok(DownloadDecisionDto {
                demand_state: demand_state.to_owned(),
                reason: "below_threshold".to_owned(),
                job: None,
            });
        }

        let season_mode = season_mode_for_release_status(&input.release_status).to_owned();

        if let Some(mut job) = db::find_open_download_job(pool, input.bangumi_subject_id).await? {
            if job.release_status != input.release_status
                || job.season_mode != season_mode
                || job.subscription_count != input.subscription_count
                || job.threshold_snapshot != input.threshold
            {
                db::update_download_job_release_context(
                    pool,
                    job.id,
                    &input.release_status,
                    &season_mode,
                    input.subscription_count,
                    input.threshold,
                )
                .await?;

                info!(
                    job_id = job.id,
                    subject_id = input.bangumi_subject_id,
                    old_release_status = %job.release_status,
                    new_release_status = %input.release_status,
                    old_season_mode = %job.season_mode,
                    new_season_mode = %season_mode,
                    subscription_count = input.subscription_count,
                    threshold = input.threshold,
                    "Updated existing download job release context before reuse"
                );

                job.release_status = input.release_status.clone();
                job.season_mode = season_mode.clone();
                job.subscription_count = input.subscription_count;
                job.threshold_snapshot = input.threshold;
            }

            return Ok(DownloadDecisionDto {
                demand_state: demand_state.to_owned(),
                reason: if input.force {
                    "reused_existing_force_job".to_owned()
                } else {
                    "reused_existing_threshold_job".to_owned()
                },
                job: Some(job),
            });
        }

        let accepted = self
            .engine
            .queue(EngineQueueRequest {
                bangumi_subject_id: input.bangumi_subject_id,
                release_status: input.release_status.clone(),
                season_mode: season_mode.clone(),
                trigger_kind: input.trigger_kind.to_owned(),
                requested_by: input.requested_by.clone(),
                subscription_count: input.subscription_count,
                threshold_snapshot: input.threshold,
            })
            .await
            .with_context(|| {
                format!(
                    "failed to queue subject {} on engine {}",
                    input.bangumi_subject_id,
                    self.engine.name()
                )
            })
            .map_err(|error| {
                warn!(
                    subject_id = input.bangumi_subject_id,
                    engine = self.engine.name(),
                    error = %error,
                    "Download engine failed to queue subject"
                );
                AppError::internal("failed to queue download job")
            })?;

        let job = db::create_download_job(
            pool,
            db::NewDownloadJob {
                bangumi_subject_id: input.bangumi_subject_id,
                trigger_kind: input.trigger_kind.to_owned(),
                requested_by: input.requested_by,
                release_status: input.release_status,
                season_mode,
                lifecycle: accepted.lifecycle,
                subscription_count: input.subscription_count,
                threshold_snapshot: input.threshold,
                engine_name: self.engine.name().to_owned(),
                engine_job_ref: accepted.engine_job_ref,
                notes: accepted.notes,
            },
        )
        .await?;

        db::mark_download_subject_queued(pool, input.bangumi_subject_id, job.id).await?;

        Ok(DownloadDecisionDto {
            demand_state: demand_state.to_owned(),
            reason: if input.force {
                "queued_force_job".to_owned()
            } else {
                "queued_threshold_job".to_owned()
            },
            job: Some(job),
        })
    }

    pub async fn list_jobs(
        &self,
        pool: &SqlitePool,
        limit: usize,
    ) -> Result<Vec<DownloadJobDto>, AppError> {
        db::list_download_jobs(pool, limit).await
    }

    pub async fn materialize_selected_candidate(
        &self,
        pool: &SqlitePool,
        media_root: &Path,
        job_id: i64,
    ) -> Result<DownloadExecutionDecisionDto, AppError> {
        self.materialize_selected_candidate_with_priority(pool, media_root, job_id, 0)
            .await
    }

    pub async fn materialize_selected_candidate_with_priority(
        &self,
        pool: &SqlitePool,
        media_root: &Path,
        job_id: i64,
        priority: u32,
    ) -> Result<DownloadExecutionDecisionDto, AppError> {
        let job = db::download_job_by_id(pool, job_id)
            .await?
            .ok_or_else(|| AppError::not_found("download job not found"))?;
        let candidate = db::current_selected_candidate_for_job(pool, job_id)
            .await?
            .ok_or_else(|| AppError::bad_request("download job has no selected candidate"))?;

        self.materialize_candidate_for_job(pool, media_root, &job, &candidate, priority)
            .await
    }

    pub async fn probe_candidate_with_priority(
        &self,
        pool: &SqlitePool,
        media_root: &Path,
        job_id: i64,
        resource_candidate_id: i64,
        priority: u32,
    ) -> Result<EngineProbeAccepted, AppError> {
        let job = db::download_job_by_id(pool, job_id)
            .await?
            .ok_or_else(|| AppError::not_found("download job not found"))?;
        let candidate = db::resource_candidate_by_id(pool, resource_candidate_id)
            .await?
            .ok_or_else(|| AppError::not_found("resource candidate not found"))?;

        if candidate.download_job_id != job.id {
            return Err(AppError::bad_request(
                "resource candidate does not belong to download job",
            ));
        }

        self.probe_candidate_for_job(pool, media_root, &job, &candidate, priority)
            .await
    }

    pub async fn materialize_candidate_with_priority(
        &self,
        pool: &SqlitePool,
        media_root: &Path,
        job_id: i64,
        resource_candidate_id: i64,
        priority: u32,
    ) -> Result<DownloadExecutionDecisionDto, AppError> {
        let job = db::download_job_by_id(pool, job_id)
            .await?
            .ok_or_else(|| AppError::not_found("download job not found"))?;
        let candidate = db::resource_candidate_by_id(pool, resource_candidate_id)
            .await?
            .ok_or_else(|| AppError::not_found("resource candidate not found"))?;

        if candidate.download_job_id != job.id {
            return Err(AppError::bad_request(
                "resource candidate does not belong to download job",
            ));
        }

        self.materialize_candidate_for_job(pool, media_root, &job, &candidate, priority)
            .await
    }

    async fn probe_candidate_for_job(
        &self,
        _pool: &SqlitePool,
        media_root: &Path,
        job: &DownloadJobDto,
        candidate: &ResourceCandidateDto,
        priority: u32,
    ) -> Result<EngineProbeAccepted, AppError> {
        let target_path = build_execution_target_path(media_root, job, candidate.id);
        ensure_execution_target_path(&target_path)?;
        let request = build_engine_activate_request(
            job,
            candidate,
            priority,
            target_path,
            "primary".to_owned(),
        );

        self.engine.probe(&request).await.map_err(|error| {
            warn!(
                job_id = job.id,
                candidate_id = candidate.id,
                slot_key = %candidate.slot_key,
                engine = self.engine.name(),
                error = %error,
                "Download engine failed to probe selected candidate"
            );
            AppError::internal("failed to probe selected resource")
        })
    }

    async fn materialize_candidate_for_job(
        &self,
        pool: &SqlitePool,
        media_root: &Path,
        job: &DownloadJobDto,
        candidate: &ResourceCandidateDto,
        priority: u32,
    ) -> Result<DownloadExecutionDecisionDto, AppError> {
        info!(
            job_id = job.id,
            subject_id = job.bangumi_subject_id,
            candidate_id = candidate.id,
            priority,
            slot_key = %candidate.slot_key,
            episode_index = ?candidate.episode_index,
            episode_end_index = ?candidate.episode_end_index,
            fansub = ?candidate.fansub_name,
            score = candidate.score,
            title = %candidate.title,
            "Preparing to materialize resource candidate into a download execution"
        );
        if let Some(existing) =
            db::find_execution_for_job_candidate(pool, job.id, candidate.id).await?
        {
            if is_active_execution_state(&existing.state) {
                info!(
                    job_id = job.id,
                    subject_id = job.bangumi_subject_id,
                    candidate_id = candidate.id,
                    execution_id = existing.id,
                    state = %existing.state,
                    "Reusing existing active execution for resource candidate"
                );
                return Ok(DownloadExecutionDecisionDto {
                    reason: "reused_existing_execution".to_owned(),
                    execution: Some(existing),
                    replaced_execution_id: None,
                });
            }
        }

        let replaced_execution =
            db::find_active_execution_for_job_slot(pool, job.id, &candidate.slot_key).await?;

        let execution_role = if replaced_execution.is_some() {
            "replacement"
        } else {
            "primary"
        }
        .to_owned();
        let target_path = build_execution_target_path(media_root, job, candidate.id);
        ensure_execution_target_path(&target_path)?;

        if let Some(previous) = replaced_execution.as_ref() {
            info!(
                job_id = job.id,
                subject_id = job.bangumi_subject_id,
                previous_execution_id = previous.id,
                previous_candidate_id = previous.resource_candidate_id,
                slot_key = %candidate.slot_key,
                "A higher-priority candidate will replace an active execution in the same slot"
            );
        }

        let activate_request = build_engine_activate_request(
            job,
            candidate,
            priority,
            target_path.clone(),
            execution_role.clone(),
        );
        let accepted = self
            .engine
            .activate(activate_request)
            .await
            .with_context(|| {
                format!(
                    "failed to activate candidate {} for download job {} on engine {}",
                    candidate.id,
                    job.id,
                    self.engine.name()
                )
            })
            .map_err(|error| {
                warn!(
                    job_id = job.id,
                    candidate_id = candidate.id,
                    slot_key = %candidate.slot_key,
                    engine = self.engine.name(),
                    error = %error,
                    "Download engine failed to activate selected candidate"
                );
                AppError::internal("failed to activate selected resource")
            })?;

        if let Some(previous) = replaced_execution.as_ref() {
            if previous.engine_name == self.engine.name()
                && previous.engine_execution_ref != accepted.engine_execution_ref
            {
                if let Err(error) = self.engine.deactivate(previous, true).await {
                    warn!(
                        execution_id = previous.id,
                        engine = %previous.engine_name,
                        error = %error,
                        "Failed to deactivate superseded execution on download engine"
                    );
                }
            }

            db::mark_download_execution_replaced(
                pool,
                previous.id,
                Some("Superseded by a higher priority resource candidate"),
            )
            .await?;
            db::delete_media_inventory_for_execution(pool, previous.id).await?;
            db::create_download_execution_event(
                pool,
                db::NewDownloadExecutionEvent {
                    download_execution_id: previous.id,
                    level: "info".to_owned(),
                    event_kind: "superseded".to_owned(),
                    message: format!(
                        "Execution superseded because candidate {} became preferred",
                        candidate.id
                    ),
                    downloaded_bytes: Some(previous.downloaded_bytes),
                    uploaded_bytes: Some(previous.uploaded_bytes),
                    download_rate_bytes: Some(previous.download_rate_bytes),
                    upload_rate_bytes: Some(previous.upload_rate_bytes),
                    peer_count: Some(previous.peer_count),
                },
            )
            .await?;
        }

        let execution = db::create_download_execution(
            pool,
            db::NewDownloadExecution {
                download_job_id: job.id,
                resource_candidate_id: candidate.id,
                bangumi_subject_id: job.bangumi_subject_id,
                slot_key: candidate.slot_key.clone(),
                episode_index: candidate.episode_index,
                episode_end_index: candidate.episode_end_index,
                is_collection: candidate.is_collection,
                engine_name: self.engine.name().to_owned(),
                engine_execution_ref: accepted.engine_execution_ref,
                execution_role,
                state: accepted.state.clone(),
                target_path,
                source_title: candidate.title.clone(),
                source_magnet: candidate.magnet.clone(),
                source_size_bytes: accepted.total_bytes.max(candidate.size_bytes),
                source_fansub_name: candidate.fansub_name.clone(),
                downloaded_bytes: accepted.downloaded_bytes,
                uploaded_bytes: accepted.uploaded_bytes,
                download_rate_bytes: accepted.download_rate_bytes,
                upload_rate_bytes: accepted.upload_rate_bytes,
                peer_count: accepted.peer_count,
                notes: accepted.notes.clone(),
            },
        )
        .await?;

        info!(
            job_id = job.id,
            subject_id = job.bangumi_subject_id,
            execution_id = execution.id,
            candidate_id = candidate.id,
            slot_key = %execution.slot_key,
            state = %execution.state,
            engine = %execution.engine_name,
            execution_ref = ?execution.engine_execution_ref,
            "Download execution created successfully"
        );

        db::create_download_execution_event(
            pool,
            db::NewDownloadExecutionEvent {
                download_execution_id: execution.id,
                level: "info".to_owned(),
                event_kind: "activated".to_owned(),
                message: format!(
                    "Execution activated from candidate {} ({})",
                    candidate.id, candidate.provider
                ),
                downloaded_bytes: Some(execution.downloaded_bytes),
                uploaded_bytes: Some(execution.uploaded_bytes),
                download_rate_bytes: Some(execution.download_rate_bytes),
                upload_rate_bytes: Some(execution.upload_rate_bytes),
                peer_count: Some(execution.peer_count),
            },
        )
        .await?;

        db::update_download_job_lifecycle(pool, job.id, &accepted.state, accepted.notes.as_deref())
            .await?;

        Ok(DownloadExecutionDecisionDto {
            reason: if replaced_execution.is_some() {
                "activated_replacement_execution".to_owned()
            } else {
                "activated_primary_execution".to_owned()
            },
            execution: Some(execution),
            replaced_execution_id: replaced_execution.map(|execution| execution.id),
        })
    }

    async fn activate_queued_candidates(
        &self,
        pool: &SqlitePool,
        media_root: &Path,
    ) -> Result<(), AppError> {
        let jobs = db::list_jobs_ready_for_activation(pool, 32).await?;

        for job in jobs {
            let decision = self
                .materialize_selected_candidate(pool, media_root, job.id)
                .await?;
            let _ = decision;
        }

        Ok(())
    }

    pub async fn list_executions(
        &self,
        pool: &SqlitePool,
        job_id: i64,
    ) -> Result<Vec<DownloadExecutionDto>, AppError> {
        db::list_download_executions(pool, job_id).await
    }

    pub async fn sync_active_executions(
        &self,
        pool: &SqlitePool,
        media_root: &Path,
    ) -> Result<(), AppError> {
        let executions = db::list_active_download_executions(pool, self.engine.name(), 256).await?;

        for execution in executions {
            match self.engine.sync_execution(&execution).await {
                Ok(snapshot) => {
                    info!(
                        execution_id = execution.id,
                        job_id = execution.download_job_id,
                        subject_id = execution.bangumi_subject_id,
                        state = %snapshot.state,
                        downloaded_bytes = snapshot.downloaded_bytes,
                        total_bytes = snapshot.total_bytes,
                        download_rate_bytes = snapshot.download_rate_bytes,
                        peer_count = snapshot.peer_count,
                        "Synchronized active download execution snapshot"
                    );
                    db::update_download_execution_metrics(
                        pool,
                        execution.id,
                        &snapshot.state,
                        snapshot.downloaded_bytes,
                        snapshot.total_bytes,
                        snapshot.uploaded_bytes,
                        snapshot.download_rate_bytes,
                        snapshot.upload_rate_bytes,
                        snapshot.peer_count,
                        snapshot.notes.as_deref(),
                    )
                    .await?;

                    if should_refresh_media_index(&execution, &snapshot.state) {
                        sync_execution_media_inventory(pool, &execution, &snapshot.state).await?;
                    }

                    if execution.state != snapshot.state {
                        db::create_download_execution_event(
                            pool,
                            db::NewDownloadExecutionEvent {
                                download_execution_id: execution.id,
                                level: event_level_for_state(&snapshot.state).to_owned(),
                                event_kind: event_kind_for_state(&snapshot.state).to_owned(),
                                message: format!(
                                    "Execution state changed from {} to {}",
                                    execution.state, snapshot.state
                                ),
                                downloaded_bytes: Some(snapshot.downloaded_bytes),
                                uploaded_bytes: Some(snapshot.uploaded_bytes),
                                download_rate_bytes: Some(snapshot.download_rate_bytes),
                                upload_rate_bytes: Some(snapshot.upload_rate_bytes),
                                peer_count: Some(snapshot.peer_count),
                            },
                        )
                        .await?;

                        db::update_download_job_lifecycle(
                            pool,
                            execution.download_job_id,
                            &snapshot.state,
                            snapshot.notes.as_deref(),
                        )
                        .await?;
                    }
                }
                Err(error) => {
                    let error_message = format!("Execution sync failed: {error:#}");
                    warn!(
                        execution_id = execution.id,
                        engine = self.engine.name(),
                        error = %error,
                        "Download execution sync failed"
                    );

                    db::update_download_execution_metrics(
                        pool,
                        execution.id,
                        "failed",
                        execution.downloaded_bytes,
                        execution.source_size_bytes.max(execution.downloaded_bytes),
                        execution.uploaded_bytes,
                        0,
                        0,
                        0,
                        Some(&error_message),
                    )
                    .await?;
                    db::create_download_execution_event(
                        pool,
                        db::NewDownloadExecutionEvent {
                            download_execution_id: execution.id,
                            level: "error".to_owned(),
                            event_kind: "sync_failed".to_owned(),
                            message: error_message.clone(),
                            downloaded_bytes: Some(execution.downloaded_bytes),
                            uploaded_bytes: Some(execution.uploaded_bytes),
                            download_rate_bytes: Some(0),
                            upload_rate_bytes: Some(0),
                            peer_count: Some(0),
                        },
                    )
                    .await?;
                    db::update_download_job_lifecycle(
                        pool,
                        execution.download_job_id,
                        "failed",
                        Some(&error_message),
                    )
                    .await?;
                }
            }
        }

        self.activate_queued_candidates(pool, media_root).await?;

        Ok(())
    }
}

fn build_engine_activate_request(
    job: &DownloadJobDto,
    candidate: &ResourceCandidateDto,
    priority: u32,
    target_path: String,
    execution_role: String,
) -> EngineActivateRequest {
    EngineActivateRequest {
        download_job_id: job.id,
        bangumi_subject_id: job.bangumi_subject_id,
        resource_candidate_id: candidate.id,
        priority,
        provider: candidate.provider.clone(),
        provider_resource_id: candidate.provider_resource_id.clone(),
        title: candidate.title.clone(),
        magnet: candidate.magnet.clone(),
        size_bytes: candidate.size_bytes,
        fansub_name: candidate.fansub_name.clone(),
        target_path,
        execution_role,
    }
}

async fn sync_execution_media_inventory(
    pool: &SqlitePool,
    execution: &DownloadExecutionDto,
    state: &str,
) -> Result<(), AppError> {
    let fallback_slot = ParsedReleaseSlot {
        slot_key: execution.slot_key.clone(),
        episode_index: execution.episode_index,
        episode_end_index: execution.episode_end_index,
        is_collection: execution.is_collection,
    };
    let status = if matches!(state, "seeding" | "completed") {
        "ready"
    } else {
        "partial"
    };
    let files =
        scan_video_files(Path::new(&execution.target_path), &fallback_slot).map_err(|error| {
            warn!(
                execution_id = execution.id,
                path = %execution.target_path,
                error = %error,
                "Failed to scan execution media files"
            );
            AppError::internal("failed to scan downloaded media files")
        })?;
    let items = files
        .into_iter()
        .map(|file| db::NewMediaInventoryItem {
            bangumi_subject_id: execution.bangumi_subject_id,
            download_job_id: execution.download_job_id,
            download_execution_id: execution.id,
            resource_candidate_id: execution.resource_candidate_id,
            slot_key: execution.slot_key.clone(),
            relative_path: file.relative_path,
            absolute_path: file.absolute_path,
            file_name: file.file_name,
            file_ext: file.file_ext,
            size_bytes: file.size_bytes,
            episode_index: file.episode_index,
            episode_end_index: file.episode_end_index,
            is_collection: file.is_collection,
            status: status.to_owned(),
        })
        .collect::<Vec<_>>();

    db::replace_media_inventory_for_execution(pool, execution.id, &items).await?;
    db::mark_download_execution_indexed(pool, execution.id).await?;
    Ok(())
}

fn should_queue_job(subscription_count: i64, threshold: i64, force: bool) -> bool {
    force || (threshold > 0 && subscription_count >= threshold)
}

fn determine_demand_state(subscription_count: i64, threshold: i64, force: bool) -> &'static str {
    if force {
        "forced"
    } else if threshold > 0 && subscription_count >= threshold {
        "threshold_met"
    } else {
        "idle"
    }
}

fn season_mode_for_release_status(release_status: &str) -> &'static str {
    match release_status {
        "airing" => "ongoing_monitor",
        "upcoming" => "upcoming_watch",
        _ => "season_pack",
    }
}

fn build_execution_target_path(
    media_root: &Path,
    job: &DownloadJobDto,
    candidate_id: i64,
) -> String {
    media_root
        .join(format!("subject-{}", job.bangumi_subject_id))
        .join(format!("job-{}", job.id))
        .join(format!("candidate-{}", candidate_id))
        .to_string_lossy()
        .into_owned()
}

fn ensure_execution_target_path(target_path: &str) -> Result<(), AppError> {
    fs::create_dir_all(target_path)
        .map_err(|_| AppError::internal("failed to prepare execution target path"))
}

fn is_active_execution_state(state: &str) -> bool {
    matches!(state, "queued" | "staged" | "starting" | "downloading" | "seeding")
}

fn should_refresh_media_index(execution: &DownloadExecutionDto, state: &str) -> bool {
    if !matches!(state, "downloading" | "seeding" | "completed") {
        return false;
    }

    if execution.state != state && matches!(state, "seeding" | "completed") {
        return true;
    }

    let Some(last_indexed_at) = execution.last_indexed_at.as_deref() else {
        return true;
    };

    let Ok(parsed) = DateTime::parse_from_rfc3339(last_indexed_at) else {
        return true;
    };

    let refresh_after = if matches!(state, "seeding" | "completed") {
        Duration::seconds(60)
    } else {
        Duration::seconds(20)
    };

    Utc::now() >= parsed.with_timezone(&Utc) + refresh_after
}

fn map_embedded_task_state(state: &EmbeddedTaskState) -> String {
    match state {
        EmbeddedTaskState::Queued => "queued",
        EmbeddedTaskState::Starting => "starting",
        EmbeddedTaskState::Downloading => "downloading",
        EmbeddedTaskState::Seeding => "seeding",
        EmbeddedTaskState::Paused => "paused",
        EmbeddedTaskState::Completed => "completed",
        EmbeddedTaskState::Failed => "failed",
        EmbeddedTaskState::Deleted => "deleted",
    }
    .to_owned()
}

fn map_rqbit_state(stats: &TorrentStats) -> String {
    match stats.state {
        TorrentStatsState::Initializing => "starting".to_owned(),
        TorrentStatsState::Live => {
            if stats.finished {
                "seeding".to_owned()
            } else {
                "downloading".to_owned()
            }
        }
        TorrentStatsState::Paused => {
            if stats.finished {
                "completed".to_owned()
            } else if stats.progress_bytes > 0 {
                "downloading".to_owned()
            } else {
                "staged".to_owned()
            }
        }
        TorrentStatsState::Error => "failed".to_owned(),
    }
}

fn rqbit_notes(stats: &TorrentStats) -> Option<String> {
    stats.error.clone().or_else(|| match stats.state {
        TorrentStatsState::Initializing => Some("Torrent metadata is initializing".to_owned()),
        TorrentStatsState::Paused if stats.finished => {
            Some("Torrent transfer is complete and currently paused".to_owned())
        }
        TorrentStatsState::Paused => Some("Torrent is paused in rqbit".to_owned()),
        _ => None,
    })
}

fn rqbit_download_rate_bytes(stats: &TorrentStats) -> i64 {
    stats
        .live
        .as_ref()
        .map(|live| mib_per_sec_to_bytes_per_sec(live.download_speed.mbps))
        .unwrap_or(0)
}

fn rqbit_upload_rate_bytes(stats: &TorrentStats) -> i64 {
    stats
        .live
        .as_ref()
        .map(|live| mib_per_sec_to_bytes_per_sec(live.upload_speed.mbps))
        .unwrap_or(0)
}

fn rqbit_peer_count(stats: &TorrentStats) -> i64 {
    stats
        .live
        .as_ref()
        .map(|live| live.snapshot.peer_stats.live as i64)
        .unwrap_or(0)
}

fn limit_mb_to_non_zero_bps(value: u64) -> Option<NonZeroU32> {
    if value == 0 {
        return None;
    }

    let bytes_per_second = value.saturating_mul(1024 * 1024).min(u32::MAX as u64) as u32;
    NonZeroU32::new(bytes_per_second)
}

fn mib_per_sec_to_bytes_per_sec(value: f64) -> i64 {
    if !value.is_finite() || value <= 0.0 {
        return 0;
    }

    let bytes = value * 1024.0 * 1024.0;
    if bytes >= i64::MAX as f64 {
        i64::MAX
    } else {
        bytes.round() as i64
    }
}

fn saturating_u64_to_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn event_kind_for_state(state: &str) -> &'static str {
    match state {
        "starting" => "engine_started",
        "downloading" => "downloading",
        "seeding" => "seeding",
        "completed" => "completed",
        "failed" => "failed",
        _ => "state_changed",
    }
}

fn event_level_for_state(state: &str) -> &'static str {
    if state == "failed" { "error" } else { "info" }
}
