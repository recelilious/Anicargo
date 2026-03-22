mod animegarden;
mod auth;
mod bangumi;
mod config;
mod db;
mod discovery;
mod downloads;
mod routes;
mod telemetry;
mod types;
mod yuc;

use anyhow::Context;
use std::sync::Arc;
use tokio::time::{self, Duration, MissedTickBehavior};
use tracing::warn;

use crate::{
    animegarden::AnimeGardenClient,
    bangumi::BangumiClient,
    config::AppConfig,
    db::connect_and_migrate,
    discovery::ResourceDiscoveryCoordinator,
    downloads::{DownloadCoordinator, PlanningDownloadEngine, RqbitDownloadEngine},
    routes::AppState,
    telemetry::RuntimeMetrics,
    yuc::YucClient,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = AppConfig::load().context("failed to load configuration")?;
    let terminal_ui_active = telemetry::should_enable_terminal_ui(&config.telemetry);
    let _telemetry_guards = telemetry::init_tracing(&config.telemetry, terminal_ui_active)
        .context("failed to initialize telemetry")?;
    let pool = connect_and_migrate(&config)
        .await
        .context("failed to initialize database")?;
    db::ensure_bootstrap_admin(&pool, &config.auth)
        .await
        .context("failed to ensure bootstrap admin")?;

    let bangumi = BangumiClient::new(&config.bangumi).context("failed to initialize bangumi")?;
    let yuc = YucClient::new(&config.yuc).context("failed to initialize yuc")?;
    let animegarden =
        AnimeGardenClient::new(&config.animegarden).context("failed to initialize animegarden")?;
    let download_engine = build_download_engine(&config)
        .await
        .context("failed to initialize download engine")?;
    let downloads = DownloadCoordinator::new(download_engine);
    let download_engine_name = downloads.engine_name().to_owned();
    let discovery = ResourceDiscoveryCoordinator::new(animegarden);
    let address = format!("{}:{}", config.server.host, config.server.port);
    let metrics = RuntimeMetrics::new(address.clone());
    let downloads_for_app = downloads.clone();
    let router = routes::build_router(AppState {
        config: config.clone(),
        pool: pool.clone(),
        bangumi,
        yuc,
        downloads: downloads_for_app,
        discovery,
        metrics: metrics.clone(),
    });
    spawn_download_sync_loop(
        downloads.clone(),
        pool.clone(),
        config.torrent.sync_interval_secs,
    );
    telemetry::spawn_terminal_dashboard(&config.telemetry, metrics, pool, download_engine_name);
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .with_context(|| format!("failed to bind server on {}", address))?;

    tracing::info!("Anicargo backend listening on http://{}", address);
    axum::serve(listener, router)
        .await
        .context("server exited unexpectedly")
}

async fn build_download_engine(
    config: &AppConfig,
) -> anyhow::Result<Arc<dyn crate::downloads::DownloadEngine>> {
    match config.torrent.engine.trim().to_ascii_lowercase().as_str() {
        "rqbit" => Ok(Arc::new(
            RqbitDownloadEngine::new(&config.storage.media_root).await?,
        )),
        "planning" => Ok(Arc::new(PlanningDownloadEngine)),
        other => {
            warn!(
                engine = other,
                "Unknown torrent engine configured; falling back to planning engine"
            );
            Ok(Arc::new(PlanningDownloadEngine))
        }
    }
}

fn spawn_download_sync_loop(
    downloads: DownloadCoordinator,
    pool: sqlx::SqlitePool,
    sync_interval_secs: u64,
) {
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(sync_interval_secs.max(1)));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            if let Err(error) = downloads.sync_active_executions(&pool).await {
                warn!(error = %error, "Download execution sync loop failed");
            }
        }
    });
}
