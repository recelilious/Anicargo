use std::{
    fs,
    io::{IsTerminal, Write},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use anyhow::Context;
use axum::{
    extract::Request, extract::State, http::StatusCode, middleware::Next, response::Response,
};
use sqlx::SqlitePool;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{config::TelemetryConfig, db};

pub struct RuntimeMetrics {
    started_at: Instant,
    server_address: String,
    request_total: AtomicU64,
    request_failures: AtomicU64,
    active_requests: AtomicU64,
    request_bytes: AtomicU64,
    response_bytes: AtomicU64,
    last_route: Arc<Mutex<String>>,
    last_status: AtomicU64,
    last_latency_ms: AtomicU64,
}

pub struct RuntimeSnapshot {
    pub uptime: Duration,
    pub server_address: String,
    pub request_total: u64,
    pub request_failures: u64,
    pub active_requests: u64,
    pub request_bytes: u64,
    pub response_bytes: u64,
    pub last_route: String,
    pub last_status: u16,
    pub last_latency_ms: u64,
}

impl RuntimeMetrics {
    pub fn new(server_address: String) -> Arc<Self> {
        Arc::new(Self {
            started_at: Instant::now(),
            server_address,
            request_total: AtomicU64::new(0),
            request_failures: AtomicU64::new(0),
            active_requests: AtomicU64::new(0),
            request_bytes: AtomicU64::new(0),
            response_bytes: AtomicU64::new(0),
            last_route: Arc::new(Mutex::new("idle".to_owned())),
            last_status: AtomicU64::new(0),
            last_latency_ms: AtomicU64::new(0),
        })
    }

    pub fn snapshot(&self) -> RuntimeSnapshot {
        RuntimeSnapshot {
            uptime: self.started_at.elapsed(),
            server_address: self.server_address.clone(),
            request_total: self.request_total.load(Ordering::Relaxed),
            request_failures: self.request_failures.load(Ordering::Relaxed),
            active_requests: self.active_requests.load(Ordering::Relaxed),
            request_bytes: self.request_bytes.load(Ordering::Relaxed),
            response_bytes: self.response_bytes.load(Ordering::Relaxed),
            last_route: self
                .last_route
                .lock()
                .map(|value| value.clone())
                .unwrap_or_else(|_| "unavailable".to_owned()),
            last_status: self.last_status.load(Ordering::Relaxed) as u16,
            last_latency_ms: self.last_latency_ms.load(Ordering::Relaxed),
        }
    }

    fn begin_request(&self, route: &str, request_bytes: u64) {
        self.request_total.fetch_add(1, Ordering::Relaxed);
        self.active_requests.fetch_add(1, Ordering::Relaxed);
        self.request_bytes
            .fetch_add(request_bytes, Ordering::Relaxed);

        if let Ok(mut last_route) = self.last_route.lock() {
            *last_route = route.to_owned();
        }
    }

    fn finish_request(&self, status: StatusCode, response_bytes: u64, latency: Duration) {
        self.active_requests.fetch_sub(1, Ordering::Relaxed);
        self.response_bytes
            .fetch_add(response_bytes, Ordering::Relaxed);
        self.last_status
            .store(status.as_u16() as u64, Ordering::Relaxed);
        self.last_latency_ms
            .store(latency.as_millis() as u64, Ordering::Relaxed);

        if status.is_client_error() || status.is_server_error() {
            self.request_failures.fetch_add(1, Ordering::Relaxed);
        }
    }
}

pub fn should_enable_terminal_ui(config: &TelemetryConfig) -> bool {
    config.enable_terminal_ui && std::io::stdout().is_terminal()
}

pub fn init_tracing(
    config: &TelemetryConfig,
    terminal_ui_active: bool,
) -> anyhow::Result<Vec<WorkerGuard>> {
    if let Some(parent) = config.log_dir.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to create telemetry parent directory {}",
                    parent.display()
                )
            })?;
        }
    }
    fs::create_dir_all(&config.log_dir).with_context(|| {
        format!(
            "failed to create telemetry log directory {}",
            config.log_dir.display()
        )
    })?;

    let file_appender = tracing_appender::rolling::daily(&config.log_dir, "anicargo.log");
    let (file_writer, file_guard) = tracing_appender::non_blocking(file_appender);

    let file_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .with_writer(file_writer)
        .with_target(true)
        .with_thread_ids(false)
        .with_thread_names(false);

    let console_layer = (!terminal_ui_active).then(|| tracing_subscriber::fmt::layer());

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("ANICARGO_LOG").unwrap_or_else(|_| "info,tower_http=info".to_owned()),
        ))
        .with(file_layer)
        .with(console_layer)
        .init();

    Ok(vec![file_guard])
}

pub async fn track_http_metrics(
    State(metrics): State<Arc<RuntimeMetrics>>,
    request: Request,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_owned();
    let route = format!("{} {}", method, path);
    let request_bytes = request
        .headers()
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);

    metrics.begin_request(&route, request_bytes);
    let started = Instant::now();
    let response = next.run(request).await;
    let response_bytes = response
        .headers()
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0);
    metrics.finish_request(response.status(), response_bytes, started.elapsed());

    response
}

pub fn spawn_terminal_dashboard(
    config: &TelemetryConfig,
    metrics: Arc<RuntimeMetrics>,
    pool: SqlitePool,
    engine_name: String,
    log_dir: PathBuf,
) {
    if !should_enable_terminal_ui(config) {
        return;
    }

    let refresh = Duration::from_secs(config.refresh_interval_secs.max(1));

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(refresh);

        loop {
            interval.tick().await;
            let snapshot = metrics.snapshot();
            let overview = db::runtime_overview(&pool).await.unwrap_or_default();
            let mut buffer = String::new();
            buffer.push_str("\x1b[2J\x1b[H");
            buffer.push_str("Anicargo Backend\n");
            buffer.push_str("================\n");
            buffer.push_str(&format!(
                "Address: {}\nUptime : {}\nEngine : {}\n\n",
                snapshot.server_address,
                format_duration(snapshot.uptime),
                engine_name
            ));

            buffer.push_str("HTTP\n");
            buffer.push_str(&format!(
                "  Active Requests : {}\n  Total Requests  : {}\n  Failed Requests : {}\n  Incoming Bytes  : {}\n  Outgoing Bytes  : {}\n  Last Route      : {}\n  Last Status     : {} ({} ms)\n\n",
                snapshot.active_requests,
                snapshot.request_total,
                snapshot.request_failures,
                human_bytes(snapshot.request_bytes),
                human_bytes(snapshot.response_bytes),
                snapshot.last_route,
                snapshot.last_status,
                snapshot.last_latency_ms,
            ));

            buffer.push_str("Runtime\n");
            buffer.push_str(&format!(
                "  Devices         : {}\n  Users           : {}\n  Active Sessions : {}\n  Subscriptions   : {}\n\n",
                overview.devices,
                overview.users,
                overview.active_sessions,
                overview.subscriptions,
            ));

            buffer.push_str("Downloads\n");
            buffer.push_str(&format!(
                "  Open Jobs       : {}\n  Selected Source : {}\n  Search Running  : {}\n  Candidates      : {}\n  Active Exec     : {}\n  DL Total        : {}\n  UL Total        : {}\n  DL Rate         : {}/s\n  UL Rate         : {}/s\n  Peers           : {}\n\n",
                overview.open_download_jobs,
                overview.jobs_with_selection,
                overview.running_searches,
                overview.resource_candidates,
                overview.active_executions,
                human_bytes(overview.downloaded_bytes as u64),
                human_bytes(overview.uploaded_bytes as u64),
                human_bytes(overview.download_rate_bytes as u64),
                human_bytes(overview.upload_rate_bytes as u64),
                overview.peer_count,
            ));

            buffer.push_str("Logs\n");
            let current_log_file = log_dir.join(format!(
                "anicargo.log.{}",
                chrono::Local::now().format("%Y-%m-%d")
            ));
            buffer.push_str(&format!(
                "  Persistent log file: {}\n",
                current_log_file.display()
            ));

            let _ = print_and_flush(&buffer);
        }
    });
}

fn print_and_flush(text: &str) -> std::io::Result<()> {
    let mut stdout = std::io::stdout();
    stdout.write_all(text.as_bytes())?;
    stdout.flush()
}

fn format_duration(duration: Duration) -> String {
    let total = duration.as_secs();
    let hours = total / 3600;
    let minutes = (total % 3600) / 60;
    let seconds = total % 60;
    format!("{hours:02}:{minutes:02}:{seconds:02}")
}

fn human_bytes(value: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];

    let mut size = value as f64;
    let mut unit = 0usize;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{} {}", value, UNITS[unit])
    } else {
        format!("{size:.2} {}", UNITS[unit])
    }
}
