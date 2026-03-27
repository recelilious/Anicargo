# Anicargo Architecture

This document explains the repository-level architecture and points to the files that currently implement each major responsibility.

## 1. Repository Shape

```text
Anicargo/
├─ backend/               Main API, schedule cache, subscription pipeline, playback
├─ frontend/web/          React web client
├─ services/downloader/   Embedded or standalone downloader runtime
├─ clients/winui3/        Future native client placeholder
├─ scripts/               Reset, log decoding, and downloader test scripts
└─ docs/                  Repository-level documentation
```

## 2. High-Level Runtime Flow

1. The backend starts, loads config, opens SQLite, ensures the bootstrap admin, starts telemetry, and starts the embedded downloader when enabled.
2. The backend keeps a cached current-season catalog from Yuc, then enriches and refreshes dynamic subject state with Bangumi.
3. The web client reads the backend API and renders guest-first pages for season browsing, search, subscriptions, history, resources, and playback.
4. When a subject reaches the subscription threshold, the backend resolves episode targets, searches AnimeGarden, scores candidates, and materializes download tasks through the downloader service.
5. Downloaded files are indexed into the media inventory, mapped back to Bangumi episodes, and streamed through HTTP Range playback.

## 3. Main Backend Entry Points

Primary startup path:

- `backend/src/main.rs`
- `backend/src/main.rs::main`

Important startup helpers:

- `backend/src/main.rs::build_download_engine`
- `backend/src/main.rs::start_optional_embedded_downloader`
- `backend/src/main.rs::spawn_optional_downloader_api`
- `backend/src/main.rs::spawn_download_sync_loop`
- `backend/src/main.rs::spawn_current_season_refresh_loop`

## 4. Backend Feature Map

### API routing and request orchestration

- `backend/src/routes.rs::build_router`
- `backend/src/routes.rs::toggle_subscription`
- `backend/src/routes.rs::resolve_download_episode_targets`
- `backend/src/routes.rs::run_download_pipeline`
- `backend/src/routes.rs::apply_airing_download_plan`
- `backend/src/routes.rs::materialize_candidate_chain_with_peer_fallback`
- `backend/src/routes.rs::build_airing_download_plan`

### Database and persistence

- `backend/src/db.rs::connect_and_migrate`
- `backend/src/db.rs::toggle_subscription`
- `backend/src/db.rs::create_download_job`
- `backend/src/db.rs::create_resource_candidate`
- `backend/src/db.rs::create_download_execution`
- `backend/src/db.rs::replace_media_inventory_for_execution`
- `backend/src/db.rs::record_playback_history`

Schema history:

- `backend/migrations/`

### Yuc-driven season catalog cache

- `backend/src/season_catalog.rs::load_current_season_calendar`
- `backend/src/season_catalog.rs::sync_current_season_catalog_now`
- `backend/src/yuc.rs`

### Additional Yuc catalog pages

- `backend/src/catalog_cache.rs::load_catalog_manifest`
- `backend/src/catalog_cache.rs::load_catalog_page`

### Bangumi integration

- `backend/src/bangumi.rs`

### AnimeGarden integration and candidate scoring

- `backend/src/animegarden.rs`
- `backend/src/discovery.rs`

### Download engine adapters

- `backend/src/downloads.rs`

### Media indexing and playback mapping

- `backend/src/media.rs::scan_video_files`
- `backend/src/routes.rs::episode_playback`
- `backend/src/routes.rs::stream_media_file`

### Runtime telemetry

- `backend/src/telemetry.rs::init_tracing`
- `backend/src/telemetry.rs::track_http_metrics`
- `backend/src/telemetry.rs::spawn_terminal_dashboard`
- `backend/src/logcodec.rs`

## 5. Frontend Feature Map

Frontend entry points:

- `frontend/web/src/main.tsx`
- `frontend/web/src/App.tsx`
- `frontend/web/src/api.ts`

Page routing:

- `/` -> `frontend/web/src/pages/SeasonPage.tsx`
- `/search` -> `frontend/web/src/pages/SearchPage.tsx`
- `/subscriptions` -> `frontend/web/src/pages/SubscriptionsPage.tsx`
- `/preview` -> `frontend/web/src/pages/YucCatalogPage.tsx`
- `/special` -> `frontend/web/src/pages/YucCatalogPage.tsx`
- `/resources` -> `frontend/web/src/pages/ResourcesPage.tsx`
- `/history` -> `frontend/web/src/pages/HistoryPage.tsx`
- `/settings` -> `frontend/web/src/pages/SettingsPage.tsx`
- `/title/:subjectId` -> `frontend/web/src/pages/SubjectPage.tsx`
- `/watch/:subjectId/:episodeId` -> `frontend/web/src/pages/WatchPage.tsx`
- `/admin` -> `frontend/web/src/pages/AdminPage.tsx`

## 6. Downloader Service Feature Map

Downloader library exports:

- `services/downloader/src/lib.rs`

Standalone binary entry:

- `services/downloader/src/main.rs`

Core service implementation:

- `services/downloader/src/service.rs::start_embedded`
- `services/downloader/src/service.rs::build_router`
- `services/downloader/src/service.rs::compute_queue_plan`
- `services/downloader/src/service.rs::compute_download_limits`
- `services/downloader/src/service.rs::compute_seed_upload_limits`
- `services/downloader/src/service.rs::fast_metadata_from_source`
- `services/downloader/src/service.rs::timeout_reason`

Configuration and task model:

- `services/downloader/src/config.rs`
- `services/downloader/src/model.rs`

## 7. Current Caching Strategy

The repository already has several local caches:

- SQLite subject and season caches in the backend database
- compact runtime logs under `backend/runtime/logs`
- downloader runtime state under `services/downloader/runtime` or the embedded runtime directory
- frontend browser-local session and appearance state

The planned dedicated filename parsing service is not part of the current architecture yet.
