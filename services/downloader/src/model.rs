use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    Download,
    Seed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Queued,
    Starting,
    Downloading,
    Seeding,
    Paused,
    Completed,
    Failed,
    Deleted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskSourceKind {
    Url,
    TorrentFile,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSource {
    pub kind: TaskSourceKind,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentFileEntry {
    pub index: usize,
    pub name: String,
    pub components: Vec<String>,
    pub length: u64,
    pub included: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TorrentMetadataSummary {
    pub info_hash: String,
    pub name: Option<String>,
    pub output_folder: String,
    pub total_bytes: u64,
    pub file_count: usize,
    pub files: Vec<TorrentFileEntry>,
    pub seen_peers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTaskRequest {
    pub kind: TaskKind,
    pub source: TaskSource,
    pub output_dir: String,
    pub priority: Option<u32>,
    pub start_enabled: Option<bool>,
    pub seed_after_download: Option<bool>,
    pub manual_download_limit_mb: Option<u64>,
    pub manual_upload_limit_mb: Option<u64>,
    pub stall_timeout_secs: Option<u64>,
    pub total_timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectTaskRequest {
    pub source: TaskSource,
    pub output_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTaskRequest {
    pub priority: Option<u32>,
    pub enabled: Option<bool>,
    pub seed_after_download: Option<bool>,
    pub manual_download_limit_mb: Option<u64>,
    pub manual_upload_limit_mb: Option<u64>,
    pub clear_manual_download_limit: Option<bool>,
    pub clear_manual_upload_limit: Option<bool>,
    pub stall_timeout_secs: Option<u64>,
    pub total_timeout_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSettingsRequest {
    pub max_concurrent_downloads: Option<usize>,
    pub max_concurrent_seeds: Option<usize>,
    pub global_download_limit_mb: Option<u64>,
    pub global_upload_limit_mb: Option<u64>,
    pub priority_decay: Option<f64>,
    pub stall_timeout_secs: Option<u64>,
    pub total_timeout_secs: Option<u64>,
    pub scheduler_interval_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSettingsDto {
    pub max_concurrent_downloads: usize,
    pub max_concurrent_seeds: usize,
    pub global_download_limit_mb: u64,
    pub global_upload_limit_mb: u64,
    pub priority_decay: f64,
    pub stall_timeout_secs: u64,
    pub total_timeout_secs: u64,
    pub scheduler_interval_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloaderTaskDto {
    pub id: Uuid,
    pub kind: TaskKind,
    pub state: TaskState,
    pub enabled: bool,
    pub priority: u32,
    pub queue_position: Option<usize>,
    pub seed_after_download: bool,
    pub source: TaskSource,
    pub output_dir: String,
    pub display_name: Option<String>,
    pub info_hash: Option<String>,
    pub metadata: Option<TorrentMetadataSummary>,
    pub engine_id: Option<String>,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub uploaded_bytes: u64,
    pub download_rate_bytes: u64,
    pub upload_rate_bytes: u64,
    pub peer_count: u32,
    pub manual_download_limit_mb: Option<u64>,
    pub manual_upload_limit_mb: Option<u64>,
    pub effective_download_limit_mb: Option<f64>,
    pub effective_upload_limit_mb: Option<f64>,
    pub stall_timeout_secs: u64,
    pub total_timeout_secs: u64,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub last_progress_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskListResponse {
    pub items: Vec<DownloaderTaskDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeOverviewResponse {
    pub started_at: DateTime<Utc>,
    pub settings: RuntimeSettingsDto,
    pub total_tasks: usize,
    pub enabled_tasks: usize,
    pub active_downloads: usize,
    pub active_seeds: usize,
    pub queued_downloads: usize,
    pub queued_seeds: usize,
    pub total_download_rate_bytes: u64,
    pub total_upload_rate_bytes: u64,
}
