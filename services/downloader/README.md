# Anicargo Downloader Service

The downloader service is the torrent runtime used by Anicargo.

It supports two integration modes:

- standalone HTTP service
- embedded Rust library used directly by the backend

Current capabilities include:

- task creation, pause, resume, update, and deletion
- torrent metadata inspection from magnet or `.torrent`
- duplicate prevention by `info_hash`
- queue planning with priorities
- global and per-task bandwidth limits
- max concurrent downloads and seeds
- stall and total timeout enforcement
- runtime snapshots for downloads and seeds

## Quick Start

```powershell
cargo run --manifest-path .\services\downloader\Cargo.toml -- --config .\services\downloader\config\downloader.example.toml
```

Default standalone address:

- `http://127.0.0.1:4010`

## Documentation

- [Deployment guide](./docs/DEPLOY.md)
- [API reference](./docs/API.md)
- [Architecture guide](./docs/ARCHITECTURE.md)
