use std::sync::Arc;

use anicargo_downloader::{DownloaderCli, DownloaderConfig, build_router, start_embedded};
use anyhow::Context;
use clap::Parser;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = DownloaderCli::parse();
    let config = DownloaderConfig::load(&cli)?;

    std::fs::create_dir_all(&config.runtime_root).with_context(|| {
        format!(
            "failed to create downloader runtime root {}",
            config.runtime_root.display()
        )
    })?;

    tracing_subscriber::registry()
        .with(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("anicargo_downloader=info,warn")),
        )
        .with(fmt::layer())
        .init();

    let runtime = start_embedded(config.clone())?;
    let service: Arc<_> = runtime.service();

    let listener = TcpListener::bind(&config.listen)
        .await
        .with_context(|| format!("failed to bind downloader listener on {}", config.listen))?;
    info!(
        listen = %config.listen,
        runtime_root = %config.runtime_root.display(),
        "Anicargo downloader service listening"
    );

    axum::serve(listener, build_router(service))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("downloader server exited unexpectedly")
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
