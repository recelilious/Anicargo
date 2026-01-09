# Anicargo

Anicargo is a local anime library manager that scans media files, parses filenames, and links them to Bangumi metadata.

## Repository Layout

- `backend/` Rust backend (API, CLI, media pipeline, metadata index)
- `frontend/` Vite + React frontend
- `assets/` static assets (future)
- `docs/` design notes (future)

## Quick Start

Fastest way to run locally:

```bash
# 1) Start PostgreSQL (optional if you already have one)
cd backend/docker/postgres
docker compose up -d

# 2) Configure
cp ../example/config.toml ../config.toml
# Edit ../config.toml:
#   media.media_dir = "/path/to/anime"
#   db.database_url = "postgres://..."

# 3) Run API
cd ..
ANICARGO_CONFIG=./config.toml cargo run -p anicargo-api

# 4) Index media (first time)
ANICARGO_CONFIG=./config.toml cargo run -p anicargo-cli -- index
```

Optional frontend:

```bash
cd frontend
npm install
npm run dev
```

More details in `backend/README.md`.
