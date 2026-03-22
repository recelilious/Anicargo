use std::{fs, path::PathBuf};

use anyhow::Context;
use clap::Parser;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub storage: StorageConfig,
    pub torrent: TorrentConfig,
    pub bangumi: BangumiConfig,
    pub yuc: YucConfig,
    pub animegarden: AnimeGardenConfig,
    pub telemetry: TelemetryConfig,
    pub auth: AuthConfig,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub database_path: PathBuf,
    pub media_root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct TorrentConfig {
    pub engine: String,
    pub sync_interval_secs: u64,
    pub max_concurrent_downloads: usize,
    pub upload_limit_mb: u64,
    pub download_limit_mb: u64,
}

#[derive(Debug, Clone)]
pub struct BangumiConfig {
    pub base_url: String,
    pub user_agent: String,
    pub request_timeout_secs: u64,
}

#[derive(Debug, Clone)]
pub struct YucConfig {
    pub base_url: String,
    pub request_timeout_secs: u64,
}

#[derive(Debug, Clone)]
pub struct AnimeGardenConfig {
    pub base_url: String,
    pub request_timeout_secs: u64,
    pub page_size: usize,
    pub max_pages: usize,
}

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub default_admin_username: String,
    pub default_admin_password: String,
    pub user_session_days: i64,
    pub admin_session_hours: i64,
}

#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    pub log_dir: PathBuf,
    pub enable_terminal_ui: bool,
    pub refresh_interval_secs: u64,
}

#[derive(Debug, Parser)]
#[command(name = "anicargo-server")]
pub struct CliArgs {
    #[arg(long)]
    pub config: Option<PathBuf>,
    #[arg(long)]
    pub host: Option<String>,
    #[arg(long)]
    pub port: Option<u16>,
    #[arg(long)]
    pub database_path: Option<PathBuf>,
    #[arg(long)]
    pub media_root: Option<PathBuf>,
    #[arg(long = "max-concurrent-downloads")]
    pub max_concurrent_downloads: Option<usize>,
    #[arg(long = "upload-limit-mb")]
    pub upload_limit_mb: Option<u64>,
    #[arg(long = "download-limit-mb")]
    pub download_limit_mb: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
struct PartialConfig {
    server: Option<PartialServerConfig>,
    storage: Option<PartialStorageConfig>,
    torrent: Option<PartialTorrentConfig>,
    bangumi: Option<PartialBangumiConfig>,
    yuc: Option<PartialYucConfig>,
    animegarden: Option<PartialAnimeGardenConfig>,
    telemetry: Option<PartialTelemetryConfig>,
    auth: Option<PartialAuthConfig>,
}

#[derive(Debug, Deserialize, Default)]
struct PartialServerConfig {
    host: Option<String>,
    port: Option<u16>,
}

#[derive(Debug, Deserialize, Default)]
struct PartialStorageConfig {
    database_path: Option<PathBuf>,
    media_root: Option<PathBuf>,
}

#[derive(Debug, Deserialize, Default)]
struct PartialTorrentConfig {
    engine: Option<String>,
    sync_interval_secs: Option<u64>,
    max_concurrent_downloads: Option<usize>,
    upload_limit_mb: Option<u64>,
    download_limit_mb: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
struct PartialBangumiConfig {
    base_url: Option<String>,
    user_agent: Option<String>,
    request_timeout_secs: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
struct PartialYucConfig {
    base_url: Option<String>,
    request_timeout_secs: Option<u64>,
}

#[derive(Debug, Deserialize, Default)]
struct PartialAnimeGardenConfig {
    base_url: Option<String>,
    request_timeout_secs: Option<u64>,
    page_size: Option<usize>,
    max_pages: Option<usize>,
}

#[derive(Debug, Deserialize, Default)]
struct PartialAuthConfig {
    default_admin_username: Option<String>,
    default_admin_password: Option<String>,
    user_session_days: Option<i64>,
    admin_session_hours: Option<i64>,
}

#[derive(Debug, Deserialize, Default)]
struct PartialTelemetryConfig {
    log_dir: Option<PathBuf>,
    enable_terminal_ui: Option<bool>,
    refresh_interval_secs: Option<u64>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_owned(),
                port: 4000,
            },
            storage: StorageConfig {
                database_path: PathBuf::from("runtime/anicargo.db"),
                media_root: PathBuf::from("runtime/media"),
            },
            torrent: TorrentConfig {
                engine: "rqbit".to_owned(),
                sync_interval_secs: 2,
                max_concurrent_downloads: 5,
                upload_limit_mb: 0,
                download_limit_mb: 5,
            },
            bangumi: BangumiConfig {
                base_url: "https://api.bgm.tv".to_owned(),
                user_agent: "Anicargo/0.1 (+https://github.com/recelilious/Anicargo)".to_owned(),
                request_timeout_secs: 15,
            },
            yuc: YucConfig {
                base_url: "https://yuc.wiki".to_owned(),
                request_timeout_secs: 10,
            },
            animegarden: AnimeGardenConfig {
                base_url: "https://api.animes.garden".to_owned(),
                request_timeout_secs: 20,
                page_size: 25,
                max_pages: 2,
            },
            telemetry: TelemetryConfig {
                log_dir: PathBuf::from("runtime/logs"),
                enable_terminal_ui: true,
                refresh_interval_secs: 1,
            },
            auth: AuthConfig {
                default_admin_username: "admin".to_owned(),
                default_admin_password: "change-me-admin".to_owned(),
                user_session_days: 14,
                admin_session_hours: 12,
            },
        }
    }
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let cli = CliArgs::parse();
        let mut config = Self::default();

        let config_path = cli
            .config
            .clone()
            .or_else(|| {
                let default_path = PathBuf::from("anicargo.toml");
                default_path.exists().then_some(default_path)
            })
            .or_else(|| {
                let backend_path = PathBuf::from("backend/config/anicargo.example.toml");
                backend_path.exists().then_some(backend_path)
            });

        if let Some(path) = config_path {
            let raw = fs::read_to_string(&path)
                .with_context(|| format!("failed to read config file at {}", path.display()))?;
            let partial = toml::from_str::<PartialConfig>(&raw)
                .with_context(|| format!("failed to parse config file at {}", path.display()))?;
            config.apply_partial(partial);
        }

        if let Some(host) = cli.host {
            config.server.host = host;
        }

        if let Some(port) = cli.port {
            config.server.port = port;
        }

        if let Some(database_path) = cli.database_path {
            config.storage.database_path = database_path;
        }

        if let Some(media_root) = cli.media_root {
            config.storage.media_root = media_root;
        }

        if let Some(max_concurrent_downloads) = cli.max_concurrent_downloads {
            config.torrent.max_concurrent_downloads = max_concurrent_downloads.max(1);
        }

        if let Some(upload_limit_mb) = cli.upload_limit_mb {
            config.torrent.upload_limit_mb = upload_limit_mb;
        }

        if let Some(download_limit_mb) = cli.download_limit_mb {
            config.torrent.download_limit_mb = download_limit_mb;
        }

        Ok(config)
    }

    fn apply_partial(&mut self, partial: PartialConfig) {
        if let Some(server) = partial.server {
            if let Some(host) = server.host {
                self.server.host = host;
            }
            if let Some(port) = server.port {
                self.server.port = port;
            }
        }

        if let Some(storage) = partial.storage {
            if let Some(database_path) = storage.database_path {
                self.storage.database_path = database_path;
            }
            if let Some(media_root) = storage.media_root {
                self.storage.media_root = media_root;
            }
        }

        if let Some(torrent) = partial.torrent {
            if let Some(engine) = torrent.engine {
                self.torrent.engine = engine;
            }
            if let Some(sync_interval_secs) = torrent.sync_interval_secs {
                self.torrent.sync_interval_secs = sync_interval_secs.max(1);
            }
            if let Some(max_concurrent_downloads) = torrent.max_concurrent_downloads {
                self.torrent.max_concurrent_downloads = max_concurrent_downloads.max(1);
            }
            if let Some(upload_limit_mb) = torrent.upload_limit_mb {
                self.torrent.upload_limit_mb = upload_limit_mb;
            }
            if let Some(download_limit_mb) = torrent.download_limit_mb {
                self.torrent.download_limit_mb = download_limit_mb;
            }
        }

        if let Some(bangumi) = partial.bangumi {
            if let Some(base_url) = bangumi.base_url {
                self.bangumi.base_url = base_url;
            }
            if let Some(user_agent) = bangumi.user_agent {
                self.bangumi.user_agent = user_agent;
            }
            if let Some(request_timeout_secs) = bangumi.request_timeout_secs {
                self.bangumi.request_timeout_secs = request_timeout_secs;
            }
        }

        if let Some(yuc) = partial.yuc {
            if let Some(base_url) = yuc.base_url {
                self.yuc.base_url = base_url;
            }
            if let Some(request_timeout_secs) = yuc.request_timeout_secs {
                self.yuc.request_timeout_secs = request_timeout_secs;
            }
        }

        if let Some(animegarden) = partial.animegarden {
            if let Some(base_url) = animegarden.base_url {
                self.animegarden.base_url = base_url;
            }
            if let Some(request_timeout_secs) = animegarden.request_timeout_secs {
                self.animegarden.request_timeout_secs = request_timeout_secs;
            }
            if let Some(page_size) = animegarden.page_size {
                self.animegarden.page_size = page_size.max(1);
            }
            if let Some(max_pages) = animegarden.max_pages {
                self.animegarden.max_pages = max_pages.max(1);
            }
        }

        if let Some(telemetry) = partial.telemetry {
            if let Some(log_dir) = telemetry.log_dir {
                self.telemetry.log_dir = log_dir;
            }
            if let Some(enable_terminal_ui) = telemetry.enable_terminal_ui {
                self.telemetry.enable_terminal_ui = enable_terminal_ui;
            }
            if let Some(refresh_interval_secs) = telemetry.refresh_interval_secs {
                self.telemetry.refresh_interval_secs = refresh_interval_secs.max(1);
            }
        }

        if let Some(auth) = partial.auth {
            if let Some(username) = auth.default_admin_username {
                self.auth.default_admin_username = username;
            }
            if let Some(password) = auth.default_admin_password {
                self.auth.default_admin_password = password;
            }
            if let Some(days) = auth.user_session_days {
                self.auth.user_session_days = days;
            }
            if let Some(hours) = auth.admin_session_hours {
                self.auth.admin_session_hours = hours;
            }
        }
    }
}
