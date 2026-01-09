use serde::Deserialize;
use std::env;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use tracing_appender::non_blocking::WorkerGuard;

const DEFAULT_BIND: &str = "0.0.0.0:3000";
const DEFAULT_TOKEN_TTL_SECS: u64 = 3600;
const DEFAULT_ADMIN_USER: &str = "admin";
const DEFAULT_ADMIN_PASSWORD: &str = "adminpwd";
const DEFAULT_INVITE_CODE: &str = "invitecode";

#[derive(Debug)]
pub enum ConfigError {
    Io(io::Error),
    Toml(toml::de::Error),
    InvalidValue(String),
    MissingValue(&'static str),
    MissingConfigFile(PathBuf),
    Logger(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::Io(err) => write!(f, "config io error: {}", err),
            ConfigError::Toml(err) => write!(f, "config parse error: {}", err),
            ConfigError::InvalidValue(message) => write!(f, "config invalid value: {}", message),
            ConfigError::MissingValue(field) => write!(f, "config missing value: {}", field),
            ConfigError::MissingConfigFile(path) => {
                write!(f, "config file not found: {}", path.display())
            }
            ConfigError::Logger(message) => write!(f, "logging init error: {}", message),
        }
    }
}

impl Error for ConfigError {}

impl From<io::Error> for ConfigError {
    fn from(err: io::Error) -> Self {
        ConfigError::Io(err)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(err: toml::de::Error) -> Self {
        ConfigError::Toml(err)
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub media: MediaConfig,
    pub hls: HlsConfig,
    pub db: DbConfig,
    pub auth: AuthConfig,
    pub server: ServerConfig,
    pub bangumi: BangumiConfig,
    pub logging: LoggingConfig,
    pub qbittorrent: QbittorrentConfig,
}

#[derive(Debug, Clone)]
pub struct MediaConfig {
    pub media_dir: Option<PathBuf>,
    pub cache_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct HlsConfig {
    pub ffmpeg_path: PathBuf,
    pub segment_secs: u32,
    pub playlist_len: u32,
    pub transcode: bool,
}

#[derive(Debug, Clone)]
pub struct DbConfig {
    pub database_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub token_ttl_secs: u64,
    pub admin_user: String,
    pub admin_password: String,
    pub invite_code: String,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub bind: String,
}

#[derive(Debug, Clone)]
pub struct BangumiConfig {
    pub access_token: Option<String>,
    pub user_agent: String,
}

#[derive(Debug, Clone)]
pub struct LoggingConfig {
    pub enabled: bool,
    pub path: PathBuf,
    pub level: String,
    pub max_total_mb: u64,
}

#[derive(Debug, Clone)]
pub struct QbittorrentConfig {
    pub base_url: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub download_dir: Option<PathBuf>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            media: MediaConfig {
                media_dir: None,
                cache_dir: PathBuf::from(".cache"),
            },
            hls: HlsConfig {
                ffmpeg_path: PathBuf::from("ffmpeg"),
                segment_secs: 6,
                playlist_len: 0,
                transcode: false,
            },
            db: DbConfig { database_url: None },
            auth: AuthConfig {
                jwt_secret: "dev-secret".to_string(),
                token_ttl_secs: DEFAULT_TOKEN_TTL_SECS,
                admin_user: DEFAULT_ADMIN_USER.to_string(),
                admin_password: DEFAULT_ADMIN_PASSWORD.to_string(),
                invite_code: DEFAULT_INVITE_CODE.to_string(),
            },
            server: ServerConfig {
                bind: DEFAULT_BIND.to_string(),
            },
            bangumi: BangumiConfig {
                access_token: None,
                user_agent: default_user_agent(),
            },
            logging: LoggingConfig {
                enabled: false,
                path: default_log_path(),
                level: "info".to_string(),
                max_total_mb: 200,
            },
            qbittorrent: QbittorrentConfig {
                base_url: "http://127.0.0.1:8080".to_string(),
                username: None,
                password: None,
                download_dir: None,
            },
        }
    }
}

impl MediaConfig {
    pub fn require_media_dir(&self) -> Result<PathBuf, ConfigError> {
        self.media_dir
            .clone()
            .ok_or(ConfigError::MissingValue("media.media_dir"))
    }
}

impl DbConfig {
    pub fn require_database_url(&self) -> Result<String, ConfigError> {
        self.database_url
            .clone()
            .ok_or(ConfigError::MissingValue("db.database_url"))
    }
}

impl AppConfig {
    pub fn load(config_override: Option<PathBuf>) -> Result<Self, ConfigError> {
        let config_from_env = env::var("ANICARGO_CONFIG").ok().map(PathBuf::from);
        let explicit_config = config_override.or(config_from_env);
        if let Some(path) = &explicit_config {
            if path.to_string_lossy().trim().is_empty() {
                return Err(ConfigError::InvalidValue(
                    "empty config path".to_string(),
                ));
            }
        }
        let explicit_requested = explicit_config.is_some();

        let config_path = if let Some(path) = explicit_config {
            Some(expand_tilde(&path))
        } else {
            let local = PathBuf::from("config.toml");
            if local.exists() {
                Some(local)
            } else if let Some(home) = home_dir() {
                let fallback = home.join(".config").join("anicargo").join("config.toml");
                if fallback.exists() {
                    Some(fallback)
                } else {
                    None
                }
            } else {
                None
            }
        };

        if explicit_requested {
            let path = config_path.as_ref().ok_or(ConfigError::MissingValue(
                "ANICARGO_CONFIG or --config",
            ))?;
            if !path.exists() {
                return Err(ConfigError::MissingConfigFile(path.clone()));
            }
        }

        let mut config = AppConfig::default();
        if let Some(path) = &config_path {
            let raw = fs::read_to_string(path)?;
            let file_config: FileConfig = toml::from_str(&raw)?;
            let base_dir = path.parent();
            config.apply_file(file_config, base_dir);
        }

        config.apply_env()?;
        config.validate()?;

        Ok(config)
    }

    fn apply_file(&mut self, file: FileConfig, base_dir: Option<&Path>) {
        if let Some(media) = file.media {
            if let Some(media_dir) = media.media_dir {
                self.media.media_dir = Some(resolve_path(base_dir, &media_dir));
            }
            if let Some(cache_dir) = media.cache_dir {
                self.media.cache_dir = resolve_path(base_dir, &cache_dir);
            }
        }

        if let Some(hls) = file.hls {
            if let Some(ffmpeg_path) = hls.ffmpeg_path {
                self.hls.ffmpeg_path = ffmpeg_path;
            }
            if let Some(segment_secs) = hls.segment_secs {
                self.hls.segment_secs = segment_secs;
            }
            if let Some(playlist_len) = hls.playlist_len {
                self.hls.playlist_len = playlist_len;
            }
            if let Some(transcode) = hls.transcode {
                self.hls.transcode = transcode;
            }
        }

        if let Some(db) = file.db {
            if let Some(database_url) = db.database_url {
                self.db.database_url = Some(database_url);
            }
        }

        if let Some(auth) = file.auth {
            if let Some(jwt_secret) = auth.jwt_secret {
                self.auth.jwt_secret = jwt_secret;
            }
            if let Some(token_ttl_secs) = auth.token_ttl_secs {
                self.auth.token_ttl_secs = token_ttl_secs;
            }
            if let Some(admin_user) = auth.admin_user {
                self.auth.admin_user = admin_user;
            }
            if let Some(admin_password) = auth.admin_password {
                self.auth.admin_password = admin_password;
            }
            if let Some(invite_code) = auth.invite_code {
                self.auth.invite_code = invite_code;
            }
        }

        if let Some(server) = file.server {
            if let Some(bind) = server.bind {
                self.server.bind = bind;
            }
        }

        if let Some(bangumi) = file.bangumi {
            if let Some(access_token) = bangumi.access_token {
                self.bangumi.access_token = Some(access_token);
            }
            if let Some(user_agent) = bangumi.user_agent {
                self.bangumi.user_agent = user_agent;
            }
        }

        if let Some(logging) = file.logging {
            if let Some(enabled) = logging.enabled {
                self.logging.enabled = enabled;
            }
            if let Some(path) = logging.path {
                self.logging.path = resolve_path(base_dir, &path);
            }
            if let Some(level) = logging.level {
                self.logging.level = level;
            }
            if let Some(max_total_mb) = logging.max_total_mb {
                self.logging.max_total_mb = max_total_mb;
            }
        }

        if let Some(qbittorrent) = file.qbittorrent {
            if let Some(base_url) = qbittorrent.base_url {
                self.qbittorrent.base_url = base_url;
            }
            if let Some(username) = qbittorrent.username {
                self.qbittorrent.username = Some(username);
            }
            if let Some(password) = qbittorrent.password {
                self.qbittorrent.password = Some(password);
            }
            if let Some(download_dir) = qbittorrent.download_dir {
                self.qbittorrent.download_dir = Some(resolve_path(base_dir, &download_dir));
            }
        }
    }

    fn apply_env(&mut self) -> Result<(), ConfigError> {
        let cwd = env::current_dir().ok();

        if let Some(value) = env_first(&["ANICARGO_MEDIA_DIR", "MEDIA_DIR"]) {
            self.media.media_dir = Some(resolve_path(cwd.as_deref(), &PathBuf::from(value)));
        }
        if let Some(value) = env_first(&["ANICARGO_CACHE_DIR", "CACHE_DIR"]) {
            self.media.cache_dir = resolve_path(cwd.as_deref(), &PathBuf::from(value));
        }
        if let Some(value) = env_first(&["ANICARGO_FFMPEG_PATH"]) {
            self.hls.ffmpeg_path = PathBuf::from(value);
        }
        if let Some(value) = env_first(&["ANICARGO_HLS_SEGMENT_SECS"]) {
            self.hls.segment_secs = parse_u32("ANICARGO_HLS_SEGMENT_SECS", &value)?;
        }
        if let Some(value) = env_first(&["ANICARGO_HLS_PLAYLIST_LEN"]) {
            self.hls.playlist_len = parse_u32("ANICARGO_HLS_PLAYLIST_LEN", &value)?;
        }
        if let Some(value) = env_first(&["ANICARGO_HLS_TRANSCODE"]) {
            self.hls.transcode = parse_bool("ANICARGO_HLS_TRANSCODE", &value)?;
        }

        if let Some(value) = env_first(&["ANICARGO_DATABASE_URL", "DATABASE_URL"]) {
            self.db.database_url = Some(value);
        }

        if let Some(value) = env_first(&["ANICARGO_BIND"]) {
            self.server.bind = value;
        }
        if let Some(value) = env_first(&["ANICARGO_ADMIN_USER"]) {
            self.auth.admin_user = value;
        }
        if let Some(value) = env_first(&["ANICARGO_ADMIN_PASSWORD"]) {
            self.auth.admin_password = value;
        }
        if let Some(value) = env_first(&["ANICARGO_INVITE_CODE"]) {
            self.auth.invite_code = value;
        }
        if let Some(value) = env_first(&["ANICARGO_JWT_SECRET"]) {
            self.auth.jwt_secret = value;
        }
        if let Some(value) = env_first(&["ANICARGO_TOKEN_TTL_SECS"]) {
            self.auth.token_ttl_secs = parse_u64("ANICARGO_TOKEN_TTL_SECS", &value)?;
        }

        if let Some(value) = env_first(&["ANICARGO_BANGUMI_ACCESS_TOKEN"]) {
            self.bangumi.access_token = Some(value);
        }
        if let Some(value) = env_first(&["ANICARGO_BANGUMI_USER_AGENT"]) {
            self.bangumi.user_agent = value;
        }

        if let Some(value) = env_first(&["ANICARGO_LOG_ENABLED"]) {
            self.logging.enabled = parse_bool("ANICARGO_LOG_ENABLED", &value)?;
        }
        if let Some(value) = env_first(&["ANICARGO_LOG_PATH"]) {
            self.logging.path = resolve_path(cwd.as_deref(), &PathBuf::from(value));
        }
        if let Some(value) = env_first(&["ANICARGO_LOG_LEVEL"]) {
            self.logging.level = value;
        }
        if let Some(value) = env_first(&["ANICARGO_LOG_MAX_MB"]) {
            self.logging.max_total_mb = parse_u64("ANICARGO_LOG_MAX_MB", &value)?;
        }

        if let Some(value) = env_first(&["ANICARGO_QBITTORRENT_BASE_URL"]) {
            self.qbittorrent.base_url = value;
        }
        if let Some(value) = env_first(&["ANICARGO_QBITTORRENT_USERNAME"]) {
            self.qbittorrent.username = Some(value);
        }
        if let Some(value) = env_first(&["ANICARGO_QBITTORRENT_PASSWORD"]) {
            self.qbittorrent.password = Some(value);
        }
        if let Some(value) = env_first(&["ANICARGO_QBITTORRENT_DOWNLOAD_DIR"]) {
            self.qbittorrent.download_dir =
                Some(resolve_path(cwd.as_deref(), &PathBuf::from(value)));
        }

        Ok(())
    }

    fn validate(&self) -> Result<(), ConfigError> {
        let media_dir = self
            .media
            .media_dir
            .as_ref()
            .ok_or(ConfigError::MissingValue("media.media_dir"))?;
        if !media_dir.is_dir() {
            return Err(ConfigError::InvalidValue(format!(
                "invalid media directory: {}",
                media_dir.display()
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, Default)]
struct FileConfig {
    media: Option<MediaConfigFile>,
    hls: Option<HlsConfigFile>,
    db: Option<DbConfigFile>,
    auth: Option<AuthConfigFile>,
    server: Option<ServerConfigFile>,
    bangumi: Option<BangumiConfigFile>,
    logging: Option<LoggingConfigFile>,
    qbittorrent: Option<QbittorrentConfigFile>,
}

#[derive(Debug, Deserialize, Default)]
struct MediaConfigFile {
    media_dir: Option<PathBuf>,
    cache_dir: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Default)]
struct HlsConfigFile {
    ffmpeg_path: Option<PathBuf>,
    segment_secs: Option<u32>,
    playlist_len: Option<u32>,
    transcode: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
struct DbConfigFile {
    database_url: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct AuthConfigFile {
    jwt_secret: Option<String>,
    token_ttl_secs: Option<u64>,
    admin_user: Option<String>,
    admin_password: Option<String>,
    invite_code: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct ServerConfigFile {
    bind: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct BangumiConfigFile {
    access_token: Option<String>,
    user_agent: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct LoggingConfigFile {
    enabled: Option<bool>,
    path: Option<PathBuf>,
    level: Option<String>,
    max_total_mb: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
struct QbittorrentConfigFile {
    base_url: Option<String>,
    username: Option<String>,
    password: Option<String>,
    download_dir: Option<PathBuf>,
}

pub fn split_config_args<I>(args: I) -> Result<(Option<PathBuf>, Vec<String>), ConfigError>
where
    I: IntoIterator<Item = String>,
{
    let mut config_path = None;
    let mut rest = Vec::new();
    let mut iter = args.into_iter();

    while let Some(arg) = iter.next() {
        if arg == "--config" {
            let value = iter.next().ok_or_else(|| {
                ConfigError::InvalidValue("missing value for --config".to_string())
            })?;
            if value.is_empty() {
                return Err(ConfigError::InvalidValue(
                    "missing value for --config".to_string(),
                ));
            }
            config_path = Some(PathBuf::from(value));
        } else if let Some(value) = arg.strip_prefix("--config=") {
            if value.is_empty() {
                return Err(ConfigError::InvalidValue(
                    "missing value for --config".to_string(),
                ));
            }
            config_path = Some(PathBuf::from(value));
        } else {
            rest.push(arg);
        }
    }

    Ok((config_path, rest))
}

pub fn init_logging(config: &LoggingConfig) -> Result<Option<WorkerGuard>, ConfigError> {
    if !config.enabled {
        return Ok(None);
    }

    fs::create_dir_all(&config.path)?;

    let level = parse_level(&config.level)?;
    let file_appender = tracing_appender::rolling::daily(&config.path, "anicargo.log");
    let (writer, guard) = tracing_appender::non_blocking(file_appender);

    let subscriber = tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_max_level(level)
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .map_err(|err| ConfigError::Logger(err.to_string()))?;

    let max_bytes = config.max_total_mb.saturating_mul(1024 * 1024);
    cleanup_log_dir(&config.path, max_bytes)?;

    Ok(Some(guard))
}

fn cleanup_log_dir(dir: &Path, max_total_bytes: u64) -> Result<(), ConfigError> {
    if max_total_bytes == 0 {
        return Ok(());
    }

    let mut entries = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if !metadata.is_file() {
            continue;
        }
        let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
        entries.push((entry.path(), modified, metadata.len()));
    }

    if entries.len() <= 1 {
        return Ok(());
    }

    entries.sort_by_key(|entry| entry.1);
    let mut total: u64 = entries.iter().map(|entry| entry.2).sum();

    let mut index = 0;
    while total > max_total_bytes && entries.len().saturating_sub(index) > 1 {
        let (path, _modified, size) = &entries[index];
        let _ = fs::remove_file(path);
        total = total.saturating_sub(*size);
        index += 1;
    }

    Ok(())
}

fn parse_level(value: &str) -> Result<tracing::Level, ConfigError> {
    match value.to_lowercase().as_str() {
        "trace" => Ok(tracing::Level::TRACE),
        "debug" => Ok(tracing::Level::DEBUG),
        "info" => Ok(tracing::Level::INFO),
        "warn" | "warning" => Ok(tracing::Level::WARN),
        "error" => Ok(tracing::Level::ERROR),
        _ => Err(ConfigError::InvalidValue(format!(
            "invalid log level: {}",
            value
        ))),
    }
}

fn parse_bool(key: &str, value: &str) -> Result<bool, ConfigError> {
    let normalized = value.trim().to_lowercase();
    match normalized.as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(ConfigError::InvalidValue(format!("invalid {}: {}", key, value))),
    }
}

fn parse_u32(key: &str, value: &str) -> Result<u32, ConfigError> {
    value
        .trim()
        .parse::<u32>()
        .map_err(|_| ConfigError::InvalidValue(format!("invalid {}: {}", key, value)))
}

fn parse_u64(key: &str, value: &str) -> Result<u64, ConfigError> {
    value
        .trim()
        .parse::<u64>()
        .map_err(|_| ConfigError::InvalidValue(format!("invalid {}: {}", key, value)))
}

fn env_first(keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Ok(value) = env::var(key) {
            return Some(value);
        }
    }
    None
}

fn resolve_path(base_dir: Option<&Path>, path: &Path) -> PathBuf {
    let expanded = expand_tilde(path);
    if expanded.is_relative() {
        if let Some(base) = base_dir {
            base.join(expanded)
        } else {
            expanded
        }
    } else {
        expanded
    }
}

fn expand_tilde(path: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    if raw == "~" {
        return home_dir().unwrap_or_else(|| path.to_path_buf());
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    }
    path.to_path_buf()
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

fn default_log_path() -> PathBuf {
    if let Some(home) = home_dir() {
        home.join(".cache").join("anicargo").join("logs")
    } else {
        PathBuf::from(".cache/anicargo/logs")
    }
}

fn default_user_agent() -> String {
    "Anicargo/0.1".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bool_accepts_common_values() {
        assert!(parse_bool("key", "true").unwrap());
        assert!(parse_bool("key", "YES").unwrap());
        assert!(parse_bool("key", "1").unwrap());
        assert!(!parse_bool("key", "false").unwrap());
        assert!(!parse_bool("key", "Off").unwrap());
        assert!(!parse_bool("key", "0").unwrap());
    }

    #[test]
    fn parse_bool_rejects_invalid_values() {
        assert!(parse_bool("key", "maybe").is_err());
        assert!(parse_bool("key", "").is_err());
    }

    #[test]
    fn parse_numbers_accept_valid_input() {
        assert_eq!(parse_u32("key", "12").unwrap(), 12);
        assert_eq!(parse_u64("key", "3600").unwrap(), 3600);
    }

    #[test]
    fn parse_numbers_reject_invalid_input() {
        assert!(parse_u32("key", "12x").is_err());
        assert!(parse_u64("key", "not").is_err());
    }

    #[test]
    fn split_config_args_extracts_path() {
        let args = vec![
            "anicargo".to_string(),
            "--config".to_string(),
            "cfg.toml".to_string(),
            "scan".to_string(),
        ];
        let (config, rest) = split_config_args(args.into_iter().skip(1)).unwrap();
        assert_eq!(config, Some(PathBuf::from("cfg.toml")));
        assert_eq!(rest, vec!["scan".to_string()]);
    }
}
