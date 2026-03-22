use std::{fs, path::Path, sync::Arc};

use anyhow::Context;
use async_trait::async_trait;
use sqlx::SqlitePool;
use tracing::{info, warn};

use crate::{
    db,
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
            uploaded_bytes: 0,
            download_rate_bytes: 0,
            upload_rate_bytes: 0,
            peer_count: 0,
        })
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

        let replaced_execution = db::find_active_execution_for_job(pool, job_id).await?;
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
            db::mark_download_execution_replaced(
                pool,
                previous.id,
                Some("Superseded by a higher priority resource candidate"),
            )
            .await?;
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
                engine_name: self.engine.name().to_owned(),
                engine_execution_ref: accepted.engine_execution_ref,
                execution_role,
                state: accepted.state.clone(),
                target_path,
                source_title: candidate.title.clone(),
                source_magnet: candidate.magnet.clone(),
                source_size_bytes: candidate.size_bytes,
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
                    "Execution staged from candidate {} ({})",
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
