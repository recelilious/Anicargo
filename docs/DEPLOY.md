# Anicargo Deployment Guide

This document describes how to run the full repository locally and what to change when moving to a LAN or server deployment.

## 1. Components

Anicargo currently has three active runtime components:

- `backend/`: the main API, scheduler, metadata cache, playback service, and management APIs.
- `frontend/web/`: the web UI.
- `services/downloader/`: the torrent engine module used by the backend, optionally exposed as its own HTTP service.

In the default setup you only need the backend and the web client. The downloader runs inside the backend process.

## 2. Requirements

Recommended minimum tooling:

- Rust stable toolchain with Cargo.
- Node.js 20+ and npm.
- Windows PowerShell or a POSIX shell.

Runtime services bundled inside the repository:

- SQLite database file.
- Embedded downloader runtime.
- Local media storage.

No external database server is required.

## 3. Fresh Local Start

Optional reset:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\reset-backend-runtime.ps1 -StopServer
```

Start the backend:

```powershell
cargo run --manifest-path .\backend\Cargo.toml -- --config .\backend\config\anicargo.example.toml
```

Start the web client:

```powershell
cd .\frontend\web
npm.cmd install
npm.cmd run dev
```

Open:

- `http://127.0.0.1:5173`
- Sign in with an admin-capable account, then open the `Manage` item in the sidebar

## 4. LAN Development

The Vite development server is already configured to listen on `0.0.0.0`.

For another device on the same LAN:

1. Start the backend with a reachable host, usually `0.0.0.0`.
2. Start the frontend dev server on the main machine.
3. Open `http://<your-lan-ip>:5173` from another device.

If you want the frontend to call a remote backend directly instead of using the Vite proxy, set:

```env
VITE_API_BASE_URL=http://<your-lan-ip>:4000
```

## 5. Backend Configuration

Configuration precedence:

1. Command-line arguments.
2. Config file.
3. Built-in defaults.

Main config file:

- `backend/config/anicargo.example.toml`

Most important backend settings:

- `server.host`
- `server.port`
- `storage.database_path`
- `storage.media_root`
- `torrent.engine`
- `torrent.max_concurrent_downloads`
- `torrent.upload_limit_mb`
- `torrent.download_limit_mb`
- `torrent.enable_service_port`
- `torrent.service_port`

## 6. Downloader Service Port

The backend always uses the downloader internally when `torrent.engine = "downloader"`.

The downloader HTTP API is optional and disabled by default. Enable it only when you need external inspection or service-level testing:

```powershell
cargo run --manifest-path .\backend\Cargo.toml -- `
  --config .\backend\config\anicargo.example.toml `
  --enable-downloader-service-port `
  --downloader-service-port 4010
```

## 7. Standalone Downloader Mode

The downloader can also run as a separate process:

```powershell
cargo run --manifest-path .\services\downloader\Cargo.toml -- `
  --config .\services\downloader\config\downloader.example.toml
```

Use this mode for isolated downloader testing or future service-level integration work.

## 8. Build Outputs And Runtime Data

Generated runtime content is intentionally outside source folders where possible:

- Backend database: `backend/runtime/anicargo.db`
- Backend media root: `backend/runtime/media`
- Backend logs: `backend/runtime/logs`
- Standalone downloader runtime: `services/downloader/runtime`

These paths are ignored by Git and should remain untracked.

## 9. Production Notes

Current recommended deployment shape:

- Linux host for the backend.
- Reverse proxy in front of the backend and static web build.
- Local filesystem for media and SQLite.

Before any public or semi-public deployment, change:

- bootstrap admin credentials
- host/port bindings
- storage paths
- reverse-proxy rules

The repository currently targets private deployment and trusted-user groups, not open public hosting.
