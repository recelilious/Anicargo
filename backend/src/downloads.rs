use std::{fs, path::Path, sync::Arc};

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

use crate::{
    db,
    media::{ParsedReleaseSlot, scan_video_files},
    types::{
        AppError, DownloadDecisionDto, DownloadExecutionDecisionDto, DownloadExecutionDto,
        DownloadJobDto,
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

#[async_trait]
pub trait DownloadEngine: Send + Sync {
    fn name(&self) -> &'static str;
    async fn queue(&self, request: EngineQueueRequest) -> anyhow::Result<EngineQueueAccepted>;
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
}

impl DownloadCoordinator {
    pub fn new(engine: Arc<dyn DownloadEngine>) -> Self {
        Self { engine }
    }

    pub fn engine_name(&self) -> &'static str {
        self.engine.name()
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

        if let Some(job) = db::find_open_download_job(pool, input.bangumi_subject_id).await? {
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

        let season_mode = match input.release_status.as_str() {
            "airing" => "ongoing_monitor",
            "upcoming" => "upcoming_watch",
            _ => "season_pack",
        }
        .to_owned();

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
        let job = db::download_job_by_id(pool, job_id)
            .await?
            .ok_or_else(|| AppError::not_found("download job not found"))?;
        let candidate = db::current_selected_candidate_for_job(pool, job_id)
            .await?
            .ok_or_else(|| AppError::bad_request("download job has no selected candidate"))?;

        if let Some(existing) =
            db::find_execution_for_job_candidate(pool, job_id, candidate.id).await?
        {
            if is_active_execution_state(&existing.state) {
                return Ok(DownloadExecutionDecisionDto {
                    reason: "reused_existing_execution".to_owned(),
                    execution: Some(existing),
                    replaced_execution_id: None,
                });
            }
        }

        let replaced_execution =
            db::find_active_execution_for_job_slot(pool, job_id, &candidate.slot_key).await?;
        let execution_role = if replaced_execution.is_some() {
            "replacement"
        } else {
            "primary"
        }
        .to_owned();
        let target_path = build_execution_target_path(media_root, &job, candidate.id);
        ensure_execution_target_path(&target_path)?;

        let accepted = self
            .engine
            .activate(EngineActivateRequest {
                download_job_id: job.id,
                bangumi_subject_id: job.bangumi_subject_id,
                resource_candidate_id: candidate.id,
                provider: candidate.provider.clone(),
                provider_resource_id: candidate.provider_resource_id.clone(),
                title: candidate.title.clone(),
                magnet: candidate.magnet.clone(),
                size_bytes: candidate.size_bytes,
                fansub_name: candidate.fansub_name.clone(),
                target_path: target_path.clone(),
                execution_role: execution_role.clone(),
            })
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

    pub async fn list_executions(
        &self,
        pool: &SqlitePool,
        job_id: i64,
    ) -> Result<Vec<DownloadExecutionDto>, AppError> {
        db::list_download_executions(pool, job_id).await
    }

    pub async fn sync_active_executions(&self, pool: &SqlitePool) -> Result<(), AppError> {
        let executions = db::list_active_download_executions(pool, self.engine.name(), 256).await?;

        for execution in executions {
            match self.engine.sync_execution(&execution).await {
                Ok(snapshot) => {
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

        Ok(())
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
    matches!(state, "staged" | "starting" | "downloading" | "seeding")
}

fn should_refresh_media_index(execution: &DownloadExecutionDto, state: &str) -> bool {
    if !matches!(state, "downloading" | "seeding" | "completed") {
        return false;
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
