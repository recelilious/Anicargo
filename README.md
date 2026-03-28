# Anicargo

Anicargo is a private-deployment anime catalog, subscription, download, and playback platform for small trusted groups.

The repository currently contains:

- A Rust backend API.
- A React + TypeScript web client.
- An embedded or standalone torrent downloader service.
- A standalone metadata parsing library for release titles and file names.
- A placeholder directory for a future WinUI 3 client.

## Quick Start

The commands below are the shortest Windows PowerShell path to a fresh local run.

1. Start the backend:

```powershell
cargo run --manifest-path .\backend\Cargo.toml -- --config .\backend\config\anicargo.example.toml
```

2. Start the web client in another terminal:

```powershell
cd .\frontend\web
npm.cmd install
npm.cmd run dev
```

3. Open the web app:

- User UI: `http://127.0.0.1:5173`
- Admin UI: `http://127.0.0.1:5173/admin`

Default bootstrap admin credentials come from the example config:

- Username: `admin`
- Password: `change-me-admin`

Optional clean reset before testing:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\reset-backend-runtime.ps1 -StopServer
```

## Repository Layout

- `backend/`: Rust API, schedule cache, subscription orchestration, media indexing, playback.
- `frontend/web/`: React web client.
- `services/downloader/`: Embedded or standalone download scheduler and torrent runtime.
- `services/metadata-parser/`: Standalone parsing library for release titles and file names.
- `clients/winui3/`: Reserved for the future native Windows client.
- `scripts/`: Maintenance, reset, and downloader test scripts.
- `docs/`: Repository-level deployment, API, and architecture documentation.

## Documentation

- [Repository deployment guide](./docs/DEPLOY.md)
- [Repository API map](./docs/API.md)
- [Repository architecture guide](./docs/ARCHITECTURE.md)
- [Backend docs](./backend/README.md)
- [Web docs](./frontend/web/README.md)
- [Downloader docs](./services/downloader/README.md)
- [Metadata parser docs](./services/metadata-parser/README.md)

## Licensing And Upstream Notes

This repository is licensed under Apache-2.0. See [LICENSE](./LICENSE).

Important upstream usage notes:

- Bangumi is used as an external API and metadata source. The `bangumi/api` repository did not expose a detectable OSS license during this audit, so treat it as an external service integration rather than code that can be copied into this repository.
- AnimeGarden repository metadata currently reports `AGPL-3.0`. This repository integrates AnimeGarden as an external API and does not vendor AnimeGarden source code.
- Yuc site pages display a `cc-by-nc-sa` badge on the homepage. Runtime-fetched Yuc-derived schedule content should be treated as attribution-required, non-commercial, share-alike content. Do not commit Yuc-derived cached data to this repository.

## Current Scope

The repository already covers:

- Current season calendar caching from Yuc plus Bangumi metadata/status enrichment.
- Guest-first viewer flow with optional user accounts and a dedicated `/admin` surface.
- Subscription-driven resource discovery and playback.
- Embedded downloader integration with an optional standalone HTTP port.
- A standalone metadata parsing library that can be embedded into backend workflows.
