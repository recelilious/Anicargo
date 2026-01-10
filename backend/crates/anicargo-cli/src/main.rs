use anicargo_bangumi::BangumiClient;
use anicargo_config::{init_logging, split_config_args, AppConfig};
use anicargo_library::{
    auto_match_all, init_library, scan_and_index, sync_bangumi_subject, AutoMatchOptions,
};
use anicargo_media::{ensure_hls, find_entry_by_id, scan_media, MediaConfig, MediaError};
use anicargo_qbittorrent::QbittorrentClient;
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::error::Error;
use std::path::Path;
use std::process;
use tracing::info;

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("error: {}", err);
        process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let (config_path, args) = split_config_args(env::args().skip(1))?;
    let mut args = args.into_iter();
    let command = args.next().unwrap_or_default();

    if matches!(command.as_str(), "help" | "") {
        print_usage();
        return Ok(());
    }
    if !matches!(
        command.as_str(),
        "scan"
            | "hls"
            | "index"
            | "bangumi-search"
            | "bangumi-sync"
            | "qbittorrent-add"
            | "qbittorrent-sync"
    ) {
        print_usage();
        return Ok(());
    }

    let app_config = AppConfig::load(config_path)?;
    let _log_guard = init_logging(&app_config.logging)?;

    let media_dir = app_config.media.require_media_dir()?;
    let media_config = MediaConfig {
        media_dir,
        cache_dir: app_config.media.cache_dir.clone(),
        ffmpeg_path: app_config.hls.ffmpeg_path.clone(),
        hls_segment_secs: app_config.hls.segment_secs,
        hls_playlist_len: app_config.hls.playlist_len,
        hls_lock_timeout_secs: app_config.hls.lock_timeout_secs,
        transcode: app_config.hls.transcode,
    };

    match command.as_str() {
        "scan" => cmd_scan(&media_config),
        "index" => cmd_index(&app_config, &media_config).await,
        "hls" => {
            let id = args
                .next()
                .ok_or_else(|| MediaError::InvalidConfig("missing media id".to_string()))?;
            cmd_hls(&media_config, &id)
        }
        "bangumi-search" => {
            let keyword = args
                .next()
                .ok_or_else(|| MediaError::InvalidConfig("missing keyword".to_string()))?;
            cmd_bangumi_search(&app_config, &keyword).await
        }
        "bangumi-sync" => {
            let subject_id = args
                .next()
                .ok_or_else(|| MediaError::InvalidConfig("missing subject id".to_string()))?;
            let subject_id = subject_id
                .parse::<i64>()
                .map_err(|_| MediaError::InvalidConfig("invalid subject id".to_string()))?;
            cmd_bangumi_sync(&app_config, subject_id).await
        }
        "qbittorrent-add" => {
            let magnet = args
                .next()
                .ok_or_else(|| MediaError::InvalidConfig("missing magnet link".to_string()))?;
            cmd_qbittorrent_add(&app_config, &magnet).await
        }
        "qbittorrent-sync" => cmd_qbittorrent_sync(&app_config, &media_config).await,
        _ => {
            print_usage();
            Ok(())
        }
    }
}

fn cmd_scan(config: &MediaConfig) -> Result<(), Box<dyn Error>> {
    let entries = scan_media(config)?;

    let count = entries.len();
    for entry in entries {
        println!("{}\t{}\t{}", entry.id, entry.size, entry.filename);
    }

    info!(count, "scan completed");
    Ok(())
}

fn cmd_hls(config: &MediaConfig, id: &str) -> Result<(), Box<dyn Error>> {
    let entry = find_entry_by_id(config, id)?;
    let session = ensure_hls(&entry, config)?;
    info!(media_id = %id, "hls generated");
    println!("{}", session.playlist_path.display());
    Ok(())
}

async fn cmd_index(
    app_config: &AppConfig,
    media_config: &MediaConfig,
) -> Result<(), Box<dyn Error>> {
    let db_url = app_config.db.require_database_url()?;
    let db = PgPoolOptions::new()
        .max_connections(app_config.db.max_connections)
        .connect(&db_url)
        .await?;

    init_library(&db).await?;
    let summary = scan_and_index(&db, media_config).await?;
    println!(
        "indexed {} files (parsed {}, skipped {}, removed {})",
        summary.upserted, summary.parsed, summary.skipped, summary.removed
    );
    Ok(())
}

async fn cmd_bangumi_search(app_config: &AppConfig, keyword: &str) -> Result<(), Box<dyn Error>> {
    let client = BangumiClient::new(
        app_config.bangumi.access_token.clone(),
        app_config.bangumi.user_agent.clone(),
    )?;
    let result = client.search_anime(keyword, 10).await?;
    for subject in result.data {
        println!("{}\t{}\t{}", subject.id, subject.name, subject.name_cn);
    }
    Ok(())
}

async fn cmd_bangumi_sync(
    app_config: &AppConfig,
    subject_id: i64,
) -> Result<(), Box<dyn Error>> {
    let client = BangumiClient::new(
        app_config.bangumi.access_token.clone(),
        app_config.bangumi.user_agent.clone(),
    )?;
    let db_url = app_config.db.require_database_url()?;
    let db = PgPoolOptions::new()
        .max_connections(app_config.db.max_connections)
        .connect(&db_url)
        .await?;
    init_library(&db).await?;
    let summary = sync_bangumi_subject(&db, &client, subject_id).await?;
    println!(
        "synced subject {} (episodes {})",
        summary.subject_id, summary.episodes
    );
    Ok(())
}

async fn cmd_qbittorrent_add(
    app_config: &AppConfig,
    value: &str,
) -> Result<(), Box<dyn Error>> {
    let client = QbittorrentClient::new(
        app_config.qbittorrent.base_url.clone(),
        app_config.qbittorrent.username.clone(),
        app_config.qbittorrent.password.clone(),
    )?;
    let save_path = app_config
        .qbittorrent
        .download_dir
        .as_ref()
        .and_then(|path| path.to_str());
    let path = Path::new(value);
    if path.exists() {
        if path.extension().and_then(|ext| ext.to_str()) != Some("torrent") {
            return Err(MediaError::InvalidConfig("file must be .torrent".to_string()).into());
        }
        client.add_torrent_file(path, save_path).await?;
        println!("queued torrent file");
        return Ok(());
    }

    client.add_magnet(value, save_path).await?;
    println!("queued magnet link");
    Ok(())
}

async fn cmd_qbittorrent_sync(
    app_config: &AppConfig,
    media_config: &MediaConfig,
) -> Result<(), Box<dyn Error>> {
    let client = QbittorrentClient::new(
        app_config.qbittorrent.base_url.clone(),
        app_config.qbittorrent.username.clone(),
        app_config.qbittorrent.password.clone(),
    )?;
    let completed = client.list_completed().await?;
    if completed.is_empty() {
        println!("no completed torrents");
        return Ok(());
    }

    let db_url = app_config.db.require_database_url()?;
    let db = PgPoolOptions::new()
        .max_connections(app_config.db.max_connections)
        .connect(&db_url)
        .await?;

    init_library(&db).await?;
    let index_summary = scan_and_index(&db, media_config).await?;
    let bangumi_client = BangumiClient::new(
        app_config.bangumi.access_token.clone(),
        app_config.bangumi.user_agent.clone(),
    )?;
    let match_summary = auto_match_all(&db, &bangumi_client, AutoMatchOptions::default()).await?;

    println!(
        "indexed {} files (parsed {}, skipped {}, removed {}), matched {} files",
        index_summary.upserted,
        index_summary.parsed,
        index_summary.skipped,
        index_summary.removed,
        match_summary.matched
    );
    Ok(())
}

fn print_usage() {
    println!("anicargo-cli");
    println!("");
    println!("Usage:");
    println!("  anicargo-cli [--config <path>] scan");
    println!("  anicargo-cli [--config <path>] index");
    println!("  anicargo-cli [--config <path>] hls <media-id>");
    println!("  anicargo-cli [--config <path>] bangumi-search <keyword>");
    println!("  anicargo-cli [--config <path>] bangumi-sync <subject-id>");
    println!("  anicargo-cli [--config <path>] qbittorrent-add <magnet|path.torrent>");
    println!("  anicargo-cli [--config <path>] qbittorrent-sync");
    println!("");
    println!("Config:");
    println!("  --config <path>      path to config.toml");
    println!("  ANICARGO_CONFIG      path to config.toml (env override)");
}
