# Backend Deployment Guide

## 1. Requirements

- Rust stable toolchain
- Cargo

The backend stores state in SQLite and local directories. No external database is required.

## 2. Start Command

```powershell
cargo run --manifest-path .\backend\Cargo.toml -- --config .\backend\config\anicargo.example.toml
```

## 3. Configuration Precedence

The backend resolves configuration in this order:

1. CLI arguments
2. TOML config file
3. Built-in defaults in `backend/src/config.rs`

Recognized config file paths:

- explicit `--config`
- `.\anicargo.toml`
- `.\backend\config\anicargo.example.toml`

## 4. Important CLI Arguments

- `--config`
- `--host`
- `--port`
- `--database-path`
- `--media-root`
- `--max-concurrent-downloads`
- `--upload-limit-mb`
- `--download-limit-mb`
- `--enable-downloader-service-port`
- `--downloader-service-port`

## 5. Config Sections

### `[server]`

- `host`
- `port`

### `[storage]`

- `database_path`
- `media_root`

### `[torrent]`

- `engine`
- `sync_interval_secs`
- `max_concurrent_downloads`
- `upload_limit_mb`
- `download_limit_mb`
- `enable_service_port`
- `service_port`

### `[bangumi]`

- `base_url`
- `user_agent`
- `request_timeout_secs`

### `[yuc]`

- `base_url`
- `request_timeout_secs`

### `[animegarden]`

- `base_url`
- `request_timeout_secs`
- `page_size`
- `max_pages`

### `[telemetry]`

- `log_dir`
- `enable_terminal_ui`
- `refresh_interval_secs`

### `[auth]`

- `default_admin_username`
- `default_admin_password`
- `user_session_days`
- `admin_session_hours`

## 6. Downloader Modes

Recommended mode:

- `torrent.engine = "downloader"`

This starts the embedded downloader runtime inside the backend process.

The downloader HTTP surface stays disabled unless you explicitly add:

```powershell
--enable-downloader-service-port
```

## 7. Runtime Output

Default runtime paths:

- Database: `backend/runtime/anicargo.db`
- Media root: `backend/runtime/media`
- Logs: `backend/runtime/logs`
- Embedded downloader runtime: `backend/runtime/media/_downloader_runtime`

## 8. Resetting Local State

From the repository root:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\reset-backend-runtime.ps1 -StopServer
```

This clears backend runtime data and frontend build artifacts so you can test a cold start.
