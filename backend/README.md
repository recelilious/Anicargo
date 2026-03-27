# Anicargo Backend

The backend is the main API and orchestration service for Anicargo.

Current responsibilities:

- Bootstrap guest, user, and admin sessions.
- Cache Yuc season catalogs and auxiliary catalog pages.
- Enrich and refresh Bangumi metadata and release status.
- Search AnimeGarden resources and score candidates.
- Trigger and monitor downloads through the embedded downloader service.
- Index finished media files and expose playback endpoints.
- Expose public, auth, and admin HTTP APIs.

## Quick Start

```powershell
cargo run --manifest-path .\backend\Cargo.toml -- --config .\backend\config\anicargo.example.toml
```

Default local address:

- `http://127.0.0.1:4000`

## Documentation

- [Deployment guide](./docs/DEPLOY.md)
- [API reference](./docs/API.md)
- [Architecture guide](./docs/ARCHITECTURE.md)
