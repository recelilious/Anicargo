mod auth;
mod bangumi;
mod config;
mod db;
mod downloads;
mod routes;
mod types;
mod yuc;

use anyhow::Context;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{
    bangumi::BangumiClient,
    config::AppConfig,
    db::connect_and_migrate,
    downloads::{DownloadCoordinator, PlanningDownloadEngine},
    routes::AppState,
    yuc::YucClient,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("ANICARGO_LOG").unwrap_or_else(|_| "info,tower_http=info".to_owned()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let config = AppConfig::load().context("failed to load configuration")?;
    let pool = connect_and_migrate(&config)
        .await
        .context("failed to initialize database")?;
    db::ensure_bootstrap_admin(&pool, &config.auth)
        .await
        .context("failed to ensure bootstrap admin")?;

    let bangumi = BangumiClient::new(&config.bangumi).context("failed to initialize bangumi")?;
    let yuc = YucClient::new(&config.yuc).context("failed to initialize yuc")?;
    let downloads = DownloadCoordinator::new(Arc::new(PlanningDownloadEngine));
    let router = routes::build_router(AppState {
        config: config.clone(),
        pool,
        bangumi,
        yuc,
        downloads,
    });

    let address = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .with_context(|| format!("failed to bind server on {}", address))?;

    tracing::info!("Anicargo backend listening on http://{}", address);
    axum::serve(listener, router)
        .await
        .context("server exited unexpectedly")
}
