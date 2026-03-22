use std::sync::Arc;

use anyhow::Context;
use async_trait::async_trait;
use sqlx::SqlitePool;
use tracing::{info, warn};

use crate::{
    db,
    types::{AppError, DownloadDecisionDto, DownloadJobDto},
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

#[async_trait]
pub trait DownloadEngine: Send + Sync {
    fn name(&self) -> &'static str;
    async fn queue(&self, request: EngineQueueRequest) -> anyhow::Result<EngineQueueAccepted>;
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
}

#[derive(Clone)]
pub struct DownloadCoordinator {
    engine: Arc<dyn DownloadEngine>,
}

impl DownloadCoordinator {
    pub fn new(engine: Arc<dyn DownloadEngine>) -> Self {
        Self { engine }
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
