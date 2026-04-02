mod animegarden;
mod auth;
mod bangumi;
mod catalog_cache;
mod config;
mod db;
mod discovery;
mod downloads;
mod logcodec;
mod media;
mod routes;
mod season_catalog;
mod telemetry;
mod types;
mod yuc;

use anicargo_downloader::{
    DownloaderConfig as EmbeddedDownloaderConfig, DownloaderRuntime as EmbeddedDownloaderRuntime,
    build_router as build_downloader_router, start_embedded as start_embedded_downloader,
};
use anyhow::Context;
use chrono::{FixedOffset, Utc};
use std::sync::Arc;
use tokio::signal;
use tokio::time::{self, Duration, MissedTickBehavior};
use tracing::warn;

use crate::{
    animegarden::AnimeGardenClient,
    bangumi::BangumiClient,
    config::AppConfig,
    db::connect_and_migrate,
    discovery::ResourceDiscoveryCoordinator,
    downloads::{
        DownloadCoordinator, DownloadRuntimeSettings, EmbeddedDownloaderEngine,
        PlanningDownloadEngine, RqbitDownloadEngine,
    },
    routes::AppState,
    telemetry::RuntimeMetrics,
    yuc::YucClient,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = AppConfig::load().context("failed to load configuration")?;
    let terminal_ui_active = telemetry::should_enable_terminal_ui(&config.telemetry);
    let (_telemetry_guards, log_file_path) =
        telemetry::init_tracing(&config.telemetry, terminal_ui_active)
            .context("failed to initialize telemetry")?;
    let pool = connect_and_migrate(&config)
        .await
        .context("failed to initialize database")?;
    db::ensure_bootstrap_admin(&pool, &config.auth)
        .await
        .context("failed to ensure bootstrap admin")?;
    db::apply_torrent_runtime_config(
        &pool,
        config.torrent.max_concurrent_downloads as i64,
        config.torrent.upload_limit_mb as i64,
        config.torrent.download_limit_mb as i64,
    )
    .await
    .context("failed to apply torrent runtime config")?;

    let bangumi = BangumiClient::new(&config.bangumi).context("failed to initialize bangumi")?;
    let yuc = YucClient::new(&config.yuc).context("failed to initialize yuc")?;
    let animegarden =
        AnimeGardenClient::new(&config.animegarden).context("failed to initialize animegarden")?;
    let download_runtime_settings = DownloadRuntimeSettings::new(
        config.torrent.max_concurrent_downloads,
        config.torrent.upload_limit_mb,
        config.torrent.download_limit_mb,
    );
    let (downloader_runtime, downloader_service) = start_optional_embedded_downloader(&config)
        .context("failed to initialize embedded downloader runtime")?;
    let download_engine = build_download_engine(&config, downloader_service.clone())
        .await
        .context("failed to initialize download engine")?;
    let downloads = DownloadCoordinator::new(download_engine, download_runtime_settings);
    downloads
        .apply_runtime_settings(download_runtime_settings)
        .await
        .context("failed to apply startup download runtime settings")?;
    let download_engine_name = downloads.engine_name().to_owned();
    let discovery = ResourceDiscoveryCoordinator::new(animegarden);
    let address = format!("{}:{}", config.server.host, config.server.port);
    let metrics = RuntimeMetrics::new(address.clone());
    let downloads_for_app = downloads.clone();
    let bangumi_for_sync = bangumi.clone();
    let yuc_for_sync = yuc.clone();
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
        config.storage.media_root.clone(),
        config.torrent.sync_interval_secs,
    );
    spawn_current_season_refresh_loop(yuc_for_sync, bangumi_for_sync, pool.clone());
    let _downloader_api_handle =
        spawn_optional_downloader_api(&config, downloader_service.clone()).await?;
    telemetry::spawn_terminal_dashboard(
        &config.telemetry,
        metrics,
        pool,
        download_engine_name,
        log_file_path,
    );
    let _downloader_runtime = downloader_runtime;
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .with_context(|| format!("failed to bind server on {}", address))?;

    tracing::info!("Anicargo backend listening on http://{}", address);
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server exited unexpectedly")?;

    tracing::info!("Anicargo backend stopped");
    Ok(())
}

async fn build_download_engine(
    config: &AppConfig,
    downloader_service: Option<Arc<anicargo_downloader::DownloaderService>>,
) -> anyhow::Result<Arc<dyn crate::downloads::DownloadEngine>> {
    match config.torrent.engine.trim().to_ascii_lowercase().as_str() {
        "downloader" | "embedded-downloader" | "embedded" => {
            let service = downloader_service
                .ok_or_else(|| anyhow::anyhow!("embedded downloader runtime is not available"))?;
            Ok(Arc::new(EmbeddedDownloaderEngine::new(service)))
        }
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

fn should_start_embedded_downloader(config: &AppConfig) -> bool {
    matches!(
        config.torrent.engine.trim().to_ascii_lowercase().as_str(),
        "downloader" | "embedded-downloader" | "embedded"
    ) || config.torrent.enable_service_port
}

fn build_embedded_downloader_config(config: &AppConfig) -> EmbeddedDownloaderConfig {
    EmbeddedDownloaderConfig {
        listen: format!("{}:{}", config.server.host, config.torrent.service_port),
        runtime_root: config.storage.media_root.join("_downloader_runtime"),
        default_output_dir: config.storage.media_root.join("_downloader_default"),
        max_concurrent_downloads: config.torrent.max_concurrent_downloads,
        max_concurrent_seeds: 8,
        global_download_limit_mb: config.torrent.download_limit_mb,
        global_upload_limit_mb: config.torrent.upload_limit_mb,
        priority_decay: 0.8,
        stall_timeout_secs: 600,
        total_timeout_secs: 14_400,
        scheduler_interval_secs: 1,
    }
}

fn start_optional_embedded_downloader(
    config: &AppConfig,
) -> anyhow::Result<(
    Option<EmbeddedDownloaderRuntime>,
    Option<Arc<anicargo_downloader::DownloaderService>>,
)> {
    if !should_start_embedded_downloader(config) {
        return Ok((None, None));
    }

    let runtime = start_embedded_downloader(build_embedded_downloader_config(config))
        .context("failed to start embedded downloader scheduler")?;
    let service = runtime.service();
    Ok((Some(runtime), Some(service)))
}

async fn spawn_optional_downloader_api(
    config: &AppConfig,
    downloader_service: Option<Arc<anicargo_downloader::DownloaderService>>,
) -> anyhow::Result<Option<tokio::task::JoinHandle<()>>> {
    if !config.torrent.enable_service_port {
        return Ok(None);
    }

    let service = downloader_service
        .ok_or_else(|| anyhow::anyhow!("embedded downloader service is unavailable"))?;
    let address = format!("{}:{}", config.server.host, config.torrent.service_port);
    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .with_context(|| format!("failed to bind embedded downloader api on {}", address))?;
    let router = build_downloader_router(service);

    tracing::info!("Embedded downloader API listening on http://{}", address);
    Ok(Some(tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, router).await {
            warn!(error = %error, "Embedded downloader API exited unexpectedly");
        }
    })))
}

fn spawn_download_sync_loop(
    downloads: DownloadCoordinator,
    pool: sqlx::SqlitePool,
    media_root: std::path::PathBuf,
    sync_interval_secs: u64,
) {
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(sync_interval_secs.max(1)));
        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            interval.tick().await;

            if let Err(error) = downloads.sync_active_executions(&pool, &media_root).await {
                warn!(error = %error, "Download execution sync loop failed");
            }
        }
    });
}

fn spawn_current_season_refresh_loop(
    yuc: YucClient,
    bangumi: BangumiClient,
    pool: sqlx::SqlitePool,
) {
    tokio::spawn(async move {
        if let Err(error) =
            season_catalog::sync_current_season_catalog_now(&yuc, &pool, &bangumi).await
        {
            warn!(error = %error, "Current season refresh loop failed during startup");
        }

        loop {
            time::sleep(next_tokyo_midnight_delay()).await;

            if let Err(error) =
                season_catalog::sync_current_season_catalog_now(&yuc, &pool, &bangumi).await
            {
                warn!(error = %error, "Current season refresh loop failed");
            }
        }
    });
}

fn next_tokyo_midnight_delay() -> Duration {
    let tokyo_offset = FixedOffset::east_opt(9 * 3600).expect("valid tokyo utc offset");
    let now_tokyo = Utc::now().with_timezone(&tokyo_offset);
    let next_day = now_tokyo
        .date_naive()
        .succ_opt()
        .expect("valid next tokyo date");
    let next_midnight = next_day
        .and_hms_opt(0, 0, 0)
        .expect("valid tokyo midnight")
        .and_local_timezone(tokyo_offset)
        .single()
        .expect("valid tokyo midnight with fixed offset")
        .with_timezone(&Utc);
    let wait_seconds = (next_midnight - Utc::now()).num_seconds().max(60) as u64;
    Duration::from_secs(wait_seconds)
}

async fn shutdown_signal() {
    #[cfg(windows)]
    {
        let ctrl_break = async {
            match signal::windows::ctrl_break() {
                Ok(mut stream) => {
                    stream.recv().await;
                }
                Err(error) => {
                    warn!(error = %error, "Failed to install Ctrl+Break shutdown handler");
                    std::future::pending::<()>().await;
                }
            }
        };

        tokio::select! {
            result = signal::ctrl_c() => {
                if let Err(error) = result {
                    warn!(error = %error, "Failed to listen for Ctrl+C shutdown signal");
                }
            }
            _ = ctrl_break => {}
        }
    }

    #[cfg(not(windows))]
    {
        let terminate = async {
            match signal::unix::signal(signal::unix::SignalKind::terminate()) {
                Ok(mut stream) => {
                    stream.recv().await;
                }
                Err(error) => {
                    warn!(error = %error, "Failed to install SIGTERM shutdown handler");
                    std::future::pending::<()>().await;
                }
            }
        };

        tokio::select! {
            result = signal::ctrl_c() => {
                if let Err(error) = result {
                    warn!(error = %error, "Failed to listen for Ctrl+C shutdown signal");
                }
            }
            _ = terminate => {}
        }
    }
}
