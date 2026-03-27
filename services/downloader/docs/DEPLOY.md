# Downloader Deployment Guide

## 1. Modes

The downloader supports:

- standalone HTTP mode
- embedded library mode

The main backend currently prefers embedded mode.

## 2. Standalone Start

```powershell
cargo run --manifest-path .\services\downloader\Cargo.toml -- --config .\services\downloader\config\downloader.example.toml
```

## 3. Configuration Precedence

1. CLI arguments
2. TOML config file
3. built-in defaults in `services/downloader/src/config.rs`

## 4. CLI Arguments

- `--config`
- `--listen`
- `--runtime-root`
- `--default-output-dir`
- `--max-concurrent-downloads`
- `--max-concurrent-seeds`
- `--global-download-limit-mb`
- `--global-upload-limit-mb`
- `--priority-decay`
- `--stall-timeout-secs`
- `--total-timeout-secs`
- `--scheduler-interval-secs`

## 5. Default Values

Defaults come from `DownloaderConfig::default()`:

- listen: `0.0.0.0:4010`
- runtime root: `runtime/downloader`
- default output dir: `runtime/downloader/downloads`
- max concurrent downloads: `5`
- max concurrent seeds: `8`
- global download limit: `0 MB/s` meaning unlimited
- global upload limit: `5 MB/s`
- priority decay: `0.8`
- stall timeout: `600s`
- total timeout: `14400s`
- scheduler interval: `2s`

## 6. Embedded Use

The downloader can run without opening a network port:

```rust
use anicargo_downloader::{DownloaderConfig, start_embedded};

let runtime = start_embedded(DownloaderConfig::default())?;
let service = runtime.service();
```

In this mode you interact with the service object directly instead of HTTP.

## 7. Manual Test Scripts

Basic smoke test:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\test-downloader-service.ps1 -BaseUrl http://127.0.0.1:4010
```

Long-running queue and throttling test:

```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\test-downloader-service-longrun.ps1 -BaseUrl http://127.0.0.1:4010
```
