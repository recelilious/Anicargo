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

use crate::{
    animegarden::AnimeGardenClient,
    bangumi::BangumiClient,
    config::AppConfig,
    db::connect_and_migrate,
    discovery::ResourceDiscoveryCoordinator,
    downloads::{DownloadCoordinator, PlanningDownloadEngine},
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
    let downloads = DownloadCoordinator::new(Arc::new(PlanningDownloadEngine));
    let download_engine_name = downloads.engine_name().to_owned();
    let discovery = ResourceDiscoveryCoordinator::new(animegarden);
    let address = format!("{}:{}", config.server.host, config.server.port);
    let metrics = RuntimeMetrics::new(address.clone());
    let router = routes::build_router(AppState {
        config: config.clone(),
        pool: pool.clone(),
        bangumi,
        yuc,
        downloads,
        discovery,
        metrics: metrics.clone(),
    });
    telemetry::spawn_terminal_dashboard(&config.telemetry, metrics, pool, download_engine_name);
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .with_context(|| format!("failed to bind server on {}", address))?;

    tracing::info!("Anicargo backend listening on http://{}", address);
    axum::serve(listener, router)
        .await
        .context("server exited unexpectedly")
}
