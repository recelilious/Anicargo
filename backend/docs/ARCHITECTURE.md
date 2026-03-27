# Backend Architecture

## 1. Startup Path

Backend startup is implemented in:

- `backend/src/main.rs::main`

The startup sequence is:

1. Load config.
2. Initialize telemetry.
3. Open SQLite and run migrations.
4. Ensure the bootstrap admin exists.
5. Apply runtime download limits.
6. Build Bangumi, Yuc, and AnimeGarden clients.
7. Start the embedded downloader when configured.
8. Build the HTTP router.
9. Start the execution sync loop and season refresh loop.
10. Optionally expose the downloader HTTP API.

## 2. Route Layer

Main router:

- `backend/src/routes.rs::build_router`

This file is intentionally the orchestration layer. It wires together:

- request authentication helpers
- database state
- metadata clients
- download coordination
- resource discovery
- response shaping for the web client

## 3. Metadata Pipeline

### Current season calendar

- `backend/src/season_catalog.rs::load_current_season_calendar`
- `backend/src/season_catalog.rs::sync_current_season_catalog_now`

Responsibilities:

- fetch Yuc current-season schedule pages
- normalize Beijing-time schedule strings into display-ready values
- cache schedule entries in SQLite
- match Yuc entries to Bangumi subjects
- refresh dynamic release status from Bangumi

### Additional catalog pages

- `backend/src/catalog_cache.rs::load_catalog_manifest`
- `backend/src/catalog_cache.rs::load_catalog_page`

Responsibilities:

- fetch and cache Yuc preview and special pages
- populate Bangumi matches and subject cache

### Bangumi client

- `backend/src/bangumi.rs`

Responsibilities:

- subject search
- subject detail fetch
- episode list fetch
- upstream error handling

## 4. Subscription-To-Download Flow

Main trigger:

- `backend/src/routes.rs::toggle_subscription`

Download planning pipeline:

- `backend/src/routes.rs::resolve_download_episode_targets`
- `backend/src/routes.rs::run_download_pipeline`
- `backend/src/routes.rs::build_airing_download_plan`
- `backend/src/routes.rs::apply_airing_download_plan`
- `backend/src/routes.rs::materialize_candidate_chain_with_peer_fallback`

Supporting modules:

- `backend/src/discovery.rs`
- `backend/src/downloads.rs`
- `backend/src/db.rs`

Current behavior:

- skip `upcoming` subjects
- resolve Bangumi episode targets
- search AnimeGarden per target episode
- score candidates with fansub and locale rules
- try candidates in descending score order
- skip candidates that probe as zero-peer
- create downloader tasks with episode-based priorities

## 5. Download Engine Boundary

The backend itself does not directly own the full torrent scheduler anymore.

Boundary module:

- `backend/src/downloads.rs`

This module wraps multiple engine modes and currently prefers:

- embedded downloader service

The embedded runtime is started from:

- `backend/src/main.rs::start_optional_embedded_downloader`

## 6. Persistence Model

Database access lives in:

- `backend/src/db.rs`

Schema changes live in:

- `backend/migrations/`

Important stored domains:

- device and user sessions
- admin sessions
- subscriptions
- policy and fansub rules
- download subjects and jobs
- resource candidates
- download executions and execution events
- media inventory
- playback history
- cached Yuc and Bangumi subject data

## 7. Media And Playback

Media indexing:

- `backend/src/media.rs::scan_video_files`

Playback routes:

- `backend/src/routes.rs::episode_playback`
- `backend/src/routes.rs::stream_media_file`

Playback behavior today:

- detect the best indexed file for the requested Bangumi episode
- expose a direct stream URL
- serve the original file via HTTP Range

## 8. Telemetry

Tracing setup:

- `backend/src/telemetry.rs::init_tracing`

HTTP metrics:

- `backend/src/telemetry.rs::track_http_metrics`

Terminal dashboard:

- `backend/src/telemetry.rs::spawn_terminal_dashboard`

Compact file-log encoding:

- `backend/src/logcodec.rs`

## 9. Current Gaps

The backend is operational but still evolving. The next major planned boundary is the dedicated filename parsing service, which is not part of the current backend architecture yet.
