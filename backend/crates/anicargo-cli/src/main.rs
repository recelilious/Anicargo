use anicargo_media::{ensure_hls, find_entry_by_id, scan_media, MediaConfig, MediaError};
use std::env;
use std::process;

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {}", err);
        process::exit(1);
    }
}

fn run() -> Result<(), MediaError> {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_default();

    match command.as_str() {
        "scan" => cmd_scan(),
        "hls" => {
            let id = args
                .next()
                .ok_or_else(|| MediaError::InvalidConfig("missing media id".to_string()))?;
            cmd_hls(&id)
        }
        "help" | "" => {
            print_usage();
            Ok(())
        }
        _ => {
            print_usage();
            Ok(())
        }
    }
}

fn cmd_scan() -> Result<(), MediaError> {
    let config = MediaConfig::from_env()?;
    let entries = scan_media(&config)?;

    for entry in entries {
        println!("{}\t{}\t{}", entry.id, entry.size, entry.filename);
    }

    Ok(())
}

fn cmd_hls(id: &str) -> Result<(), MediaError> {
    let config = MediaConfig::from_env()?;
    let entry = find_entry_by_id(&config, id)?;
    let session = ensure_hls(&entry, &config)?;
    println!("{}", session.playlist_path.display());
    Ok(())
}

fn print_usage() {
    println!("anicargo-cli");
    println!("");
    println!("Usage:");
    println!("  anicargo-cli scan");
    println!("  anicargo-cli hls <media-id>");
    println!("");
    println!("Environment:");
    println!("  ANICARGO_MEDIA_DIR   path to media folder");
    println!("  ANICARGO_CACHE_DIR   output cache directory (default: .cache)");
    println!("  ANICARGO_FFMPEG_PATH path to ffmpeg binary (default: ffmpeg)");
}
