use serde::Serialize;
use std::collections::hash_map::DefaultHasher;
use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const DEFAULT_CACHE_DIR: &str = ".cache";
const DEFAULT_FFMPEG_PATH: &str = "ffmpeg";
const DEFAULT_HLS_SEGMENT_SECS: u32 = 6;
const DEFAULT_HLS_PLAYLIST_LEN: u32 = 0;

#[derive(Debug, Clone)]
pub struct MediaConfig {
    pub media_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub ffmpeg_path: PathBuf,
    pub hls_segment_secs: u32,
    pub hls_playlist_len: u32,
    pub transcode: bool,
}

impl MediaConfig {
    pub fn new(media_dir: PathBuf, cache_dir: PathBuf) -> Self {
        Self {
            media_dir,
            cache_dir,
            ffmpeg_path: PathBuf::from(DEFAULT_FFMPEG_PATH),
            hls_segment_secs: DEFAULT_HLS_SEGMENT_SECS,
            hls_playlist_len: DEFAULT_HLS_PLAYLIST_LEN,
            transcode: false,
        }
    }

    pub fn from_env() -> Result<Self, MediaError> {
        let media_dir = env::var("ANICARGO_MEDIA_DIR")
            .or_else(|_| env::var("MEDIA_DIR"))
            .map(PathBuf::from)
            .map_err(|_| MediaError::MissingMediaDir)?;

        if !media_dir.is_dir() {
            return Err(MediaError::InvalidMediaDir(media_dir));
        }

        let cache_dir = env::var("ANICARGO_CACHE_DIR")
            .or_else(|_| env::var("CACHE_DIR"))
            .unwrap_or_else(|_| DEFAULT_CACHE_DIR.to_string());

        let ffmpeg_path = env::var("ANICARGO_FFMPEG_PATH")
            .unwrap_or_else(|_| DEFAULT_FFMPEG_PATH.to_string());

        let hls_segment_secs = parse_env_u32("ANICARGO_HLS_SEGMENT_SECS", DEFAULT_HLS_SEGMENT_SECS)?;
        let hls_playlist_len = parse_env_u32("ANICARGO_HLS_PLAYLIST_LEN", DEFAULT_HLS_PLAYLIST_LEN)?;
        let transcode = parse_env_bool("ANICARGO_HLS_TRANSCODE", false)?;

        Ok(Self {
            media_dir,
            cache_dir: PathBuf::from(cache_dir),
            ffmpeg_path: PathBuf::from(ffmpeg_path),
            hls_segment_secs,
            hls_playlist_len,
            transcode,
        })
    }

    pub fn hls_root(&self) -> PathBuf {
        self.cache_dir.join("hls")
    }
}

fn parse_env_u32(key: &str, default_value: u32) -> Result<u32, MediaError> {
    match env::var(key) {
        Ok(value) => value
            .parse::<u32>()
            .map_err(|_| MediaError::InvalidConfig(format!("invalid {}: {}", key, value))),
        Err(_) => Ok(default_value),
    }
}

fn parse_env_bool(key: &str, default_value: bool) -> Result<bool, MediaError> {
    match env::var(key) {
        Ok(value) => {
            let normalized = value.to_lowercase();
            match normalized.as_str() {
                "1" | "true" | "yes" | "on" => Ok(true),
                "0" | "false" | "no" | "off" => Ok(false),
                _ => Err(MediaError::InvalidConfig(format!("invalid {}: {}", key, value))),
            }
        }
        Err(_) => Ok(default_value),
    }
}

#[derive(Debug)]
pub enum MediaError {
    Io(io::Error),
    MissingMediaDir,
    InvalidMediaDir(PathBuf),
    InvalidConfig(String),
    NotFound(String),
}

impl fmt::Display for MediaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MediaError::Io(err) => write!(f, "io error: {}", err),
            MediaError::MissingMediaDir => write!(f, "missing media directory (ANICARGO_MEDIA_DIR)"),
            MediaError::InvalidMediaDir(path) => {
                write!(f, "invalid media directory: {}", path.display())
            }
            MediaError::InvalidConfig(message) => write!(f, "invalid config: {}", message),
            MediaError::NotFound(message) => write!(f, "not found: {}", message),
        }
    }
}

impl Error for MediaError {}

impl From<io::Error> for MediaError {
    fn from(err: io::Error) -> Self {
        MediaError::Io(err)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MediaEntry {
    pub id: String,
    pub filename: String,
    pub size: u64,
    #[serde(skip_serializing)]
    pub path: PathBuf,
}

impl MediaEntry {
    pub fn from_path(path: &Path) -> Result<Self, MediaError> {
        let metadata = fs::metadata(path)?;
        let filename = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| MediaError::InvalidConfig("invalid filename".to_string()))?
            .to_string();

        Ok(Self {
            id: media_id_from_path(path),
            filename,
            size: metadata.len(),
            path: path.to_path_buf(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct HlsSession {
    pub id: String,
    pub output_dir: PathBuf,
    pub playlist_path: PathBuf,
}

pub fn scan_media(config: &MediaConfig) -> Result<Vec<MediaEntry>, MediaError> {
    let mut entries = Vec::new();

    for entry in fs::read_dir(&config.media_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let ext = match path.extension().and_then(|ext| ext.to_str()) {
            Some(value) => value.to_lowercase(),
            None => continue,
        };

        if !is_media_extension(&ext) {
            continue;
        }

        entries.push(MediaEntry::from_path(&path)?);
    }

    entries.sort_by(|a, b| a.filename.cmp(&b.filename));
    Ok(entries)
}

pub fn find_entry_by_id(config: &MediaConfig, id: &str) -> Result<MediaEntry, MediaError> {
    let entries = scan_media(config)?;
    entries
        .into_iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| MediaError::NotFound(format!("media id {}", id)))
}

pub fn ensure_hls(entry: &MediaEntry, config: &MediaConfig) -> Result<HlsSession, MediaError> {
    let output_dir = hls_output_dir(config, &entry.id);
    fs::create_dir_all(&output_dir)?;

    let playlist_path = output_dir.join("index.m3u8");
    if !playlist_path.exists() {
        spawn_ffmpeg_hls(&entry.path, &output_dir, config)?;
    }

    Ok(HlsSession {
        id: entry.id.clone(),
        output_dir,
        playlist_path,
    })
}

fn spawn_ffmpeg_hls(input: &Path, output_dir: &Path, config: &MediaConfig) -> Result<(), MediaError> {
    let segment_pattern = output_dir.join("segment_%05d.ts");
    let playlist_path = output_dir.join("index.m3u8");

    let mut cmd = Command::new(&config.ffmpeg_path);
    cmd.arg("-y")
        .arg("-i")
        .arg(input);

    if config.transcode {
        cmd.args([
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "23",
            "-c:a",
            "aac",
            "-b:a",
            "128k",
        ]);
    } else {
        cmd.args(["-c", "copy"]);
    }

    cmd.arg("-start_number")
        .arg("0")
        .arg("-hls_time")
        .arg(config.hls_segment_secs.to_string())
        .arg("-hls_list_size")
        .arg(config.hls_playlist_len.to_string())
        .arg("-hls_playlist_type")
        .arg("vod")
        .arg("-hls_flags")
        .arg("independent_segments")
        .arg("-hls_segment_filename")
        .arg(segment_pattern)
        .arg("-f")
        .arg("hls")
        .arg(playlist_path)
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let _child = cmd.spawn()?;
    Ok(())
}

fn hls_output_dir(config: &MediaConfig, id: &str) -> PathBuf {
    config.hls_root().join(id)
}

fn is_media_extension(ext: &str) -> bool {
    matches!(ext, "mp4" | "mkv")
}

fn media_id_from_path(path: &Path) -> String {
    let mut hasher = DefaultHasher::new();
    path.to_string_lossy().hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
