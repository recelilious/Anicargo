use std::path::PathBuf;

use anyhow::Context;
use clap::Parser;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Parser)]
#[command(
    name = "anicargo-downloader",
    about = "Standalone torrent downloader service for Anicargo"
)]
pub struct DownloaderCli {
    #[arg(long)]
    pub config: Option<PathBuf>,
    #[arg(long)]
    pub listen: Option<String>,
    #[arg(long)]
    pub runtime_root: Option<PathBuf>,
    #[arg(long)]
    pub default_output_dir: Option<PathBuf>,
    #[arg(long)]
    pub max_concurrent_downloads: Option<usize>,
    #[arg(long)]
    pub max_concurrent_seeds: Option<usize>,
    #[arg(long)]
    pub global_download_limit_mb: Option<u64>,
    #[arg(long)]
    pub global_upload_limit_mb: Option<u64>,
    #[arg(long)]
    pub priority_decay: Option<f64>,
    #[arg(long)]
    pub stall_timeout_secs: Option<u64>,
    #[arg(long)]
    pub total_timeout_secs: Option<u64>,
    #[arg(long)]
    pub scheduler_interval_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloaderConfig {
    pub listen: String,
    pub runtime_root: PathBuf,
    pub default_output_dir: PathBuf,
    pub max_concurrent_downloads: usize,
    pub max_concurrent_seeds: usize,
    pub global_download_limit_mb: u64,
    pub global_upload_limit_mb: u64,
    pub priority_decay: f64,
    pub stall_timeout_secs: u64,
    pub total_timeout_secs: u64,
    pub scheduler_interval_secs: u64,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct FileConfig {
    listen: Option<String>,
    runtime_root: Option<PathBuf>,
    default_output_dir: Option<PathBuf>,
    max_concurrent_downloads: Option<usize>,
    max_concurrent_seeds: Option<usize>,
    global_download_limit_mb: Option<u64>,
    global_upload_limit_mb: Option<u64>,
    priority_decay: Option<f64>,
    stall_timeout_secs: Option<u64>,
    total_timeout_secs: Option<u64>,
    scheduler_interval_secs: Option<u64>,
}

impl Default for DownloaderConfig {
    fn default() -> Self {
        Self {
            listen: "0.0.0.0:4010".to_owned(),
            runtime_root: PathBuf::from("runtime/downloader"),
            default_output_dir: PathBuf::from("runtime/downloader/downloads"),
            max_concurrent_downloads: 5,
            max_concurrent_seeds: 8,
            global_download_limit_mb: 0,
            global_upload_limit_mb: 5,
            priority_decay: 0.8,
            stall_timeout_secs: 600,
            total_timeout_secs: 14_400,
            scheduler_interval_secs: 1,
        }
    }
}

impl DownloaderConfig {
    pub fn load(cli: &DownloaderCli) -> anyhow::Result<Self> {
        let mut config = DownloaderConfig::default();

        if let Some(path) = cli.config.as_ref() {
            let raw = std::fs::read_to_string(path)
                .with_context(|| format!("failed to read downloader config {}", path.display()))?;
            let file_config = toml::from_str::<FileConfig>(&raw)
                .with_context(|| format!("failed to parse downloader config {}", path.display()))?;
            config.apply_file(file_config);
        }

        config.apply_cli(cli);
        config.sanitize();
        Ok(config)
    }

    fn apply_file(&mut self, file: FileConfig) {
        if let Some(value) = file.listen {
            self.listen = value;
        }
        if let Some(value) = file.runtime_root {
            self.runtime_root = value;
        }
        if let Some(value) = file.default_output_dir {
            self.default_output_dir = value;
        }
        if let Some(value) = file.max_concurrent_downloads {
            self.max_concurrent_downloads = value;
        }
        if let Some(value) = file.max_concurrent_seeds {
            self.max_concurrent_seeds = value;
        }
        if let Some(value) = file.global_download_limit_mb {
            self.global_download_limit_mb = value;
        }
        if let Some(value) = file.global_upload_limit_mb {
            self.global_upload_limit_mb = value;
        }
        if let Some(value) = file.priority_decay {
            self.priority_decay = value;
        }
        if let Some(value) = file.stall_timeout_secs {
            self.stall_timeout_secs = value;
        }
        if let Some(value) = file.total_timeout_secs {
            self.total_timeout_secs = value;
        }
        if let Some(value) = file.scheduler_interval_secs {
            self.scheduler_interval_secs = value;
        }
    }

    fn apply_cli(&mut self, cli: &DownloaderCli) {
        if let Some(value) = cli.listen.as_ref() {
            self.listen = value.clone();
        }
        if let Some(value) = cli.runtime_root.as_ref() {
            self.runtime_root = value.clone();
        }
        if let Some(value) = cli.default_output_dir.as_ref() {
            self.default_output_dir = value.clone();
        }
        if let Some(value) = cli.max_concurrent_downloads {
            self.max_concurrent_downloads = value;
        }
        if let Some(value) = cli.max_concurrent_seeds {
            self.max_concurrent_seeds = value;
        }
        if let Some(value) = cli.global_download_limit_mb {
            self.global_download_limit_mb = value;
        }
        if let Some(value) = cli.global_upload_limit_mb {
            self.global_upload_limit_mb = value;
        }
        if let Some(value) = cli.priority_decay {
            self.priority_decay = value;
        }
        if let Some(value) = cli.stall_timeout_secs {
            self.stall_timeout_secs = value;
        }
        if let Some(value) = cli.total_timeout_secs {
            self.total_timeout_secs = value;
        }
        if let Some(value) = cli.scheduler_interval_secs {
            self.scheduler_interval_secs = value;
        }
    }

    fn sanitize(&mut self) {
        if self.default_output_dir.as_os_str().is_empty() {
            self.default_output_dir = self.runtime_root.join("downloads");
        }
        self.max_concurrent_downloads = self.max_concurrent_downloads.max(1);
        self.max_concurrent_seeds = self.max_concurrent_seeds.max(1);
        self.priority_decay = self.priority_decay.clamp(0.01, 1.0);
        self.stall_timeout_secs = self.stall_timeout_secs.max(60);
        self.total_timeout_secs = self.total_timeout_secs.max(self.stall_timeout_secs);
        self.scheduler_interval_secs = self.scheduler_interval_secs.clamp(1, 30);
    }
}
