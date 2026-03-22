mod animegarden;
mod auth;
mod bangumi;
mod config;
mod db;
mod discovery;
mod downloads;
mod media;
mod routes;
mod season_catalog;
mod telemetry;
mod types;
mod yuc;

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
        config.torrent.sync_interval_secs,
    );
    spawn_current_season_refresh_loop(yuc_for_sync, bangumi_for_sync, pool.clone());
    telemetry::spawn_terminal_dashboard(
        &config.telemetry,
        metrics,
        pool,
        download_engine_name,
        config.telemetry.log_dir.clone(),
    );
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
