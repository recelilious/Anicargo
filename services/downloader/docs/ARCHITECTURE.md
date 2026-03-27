# Downloader Architecture

## 1. Entry Points

Library exports:

- `services/downloader/src/lib.rs`

Standalone process entry:

- `services/downloader/src/main.rs`

## 2. Core Service Startup

Embedded runtime bootstrap:

- `services/downloader/src/service.rs::start_embedded`

This creates:

- shared service state
- scheduler loop
- rqbit-backed sessions
- runtime directories

## 3. HTTP Surface

The standalone HTTP API is built by:

- `services/downloader/src/service.rs::build_router`

The router exposes health, settings, inspect, task, download, and seed endpoints.

## 4. Scheduling Core

The queue planner lives in:

- `services/downloader/src/service.rs::compute_queue_plan`
- `services/downloader/src/service.rs::compute_download_limits`
- `services/downloader/src/service.rs::compute_seed_upload_limits`

Current planning rules:

- smaller numeric priority wins
- older tasks win within the same priority
- manual per-task limits are subtracted from global bandwidth before layered allocation
- stalled tasks are removed from active slots after timeout

## 5. Metadata And Deduplication

Fast metadata path:

- `services/downloader/src/service.rs::fast_metadata_from_source`
- `services/downloader/src/service.rs::fast_metadata_from_magnet`

Normalization helpers:

- `services/downloader/src/service.rs::normalize_btih`

Task deduplication is based on normalized `info_hash` rather than only the original magnet string.

## 6. Timeout Logic

Timeout decision:

- `services/downloader/src/service.rs::timeout_reason`

Current enforced timeout families:

- stall timeout
- total runtime timeout

## 7. Configuration Model

Config definitions:

- `services/downloader/src/config.rs`

Task and API models:

- `services/downloader/src/model.rs`

## 8. Integration Strategy

The downloader is designed to be used in both modes:

- embedded library calls from another Rust process
- external HTTP control when service-level testing is needed

This dual-use boundary is what allows the main backend to keep downloader logic internal by default while still preserving an isolated test surface.
