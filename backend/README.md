# Anicargo Backend

This backend is split into a media library, a CLI, and an API server. The media library owns scanning and HLS output; the API only exposes HTTP endpoints.

## Prerequisites

- Rust (stable)
- ffmpeg (available in PATH, or set `ANICARGO_FFMPEG_PATH`)
- C++ toolchain (required by the Anitomy filename parser)

## Configuration

You can provide a `config.toml`. Load order:
- `--config <path>` (CLI flag) or `ANICARGO_CONFIG`
- `./config.toml`
- `~/.config/anicargo/config.toml`

Defaults are used when missing; environment variables override config file values.

Config file example:

```toml
[media]
media_dir = "/data/anime"
cache_dir = ".cache"

[hls]
ffmpeg_path = "ffmpeg"
segment_secs = 6
playlist_len = 0
lock_timeout_secs = 3600
transcode = false

[db]
database_url = "postgres://anicargo:anicargo@127.0.0.1:5432/anicargo"
max_connections = 5

[auth]
jwt_secret = "dev-secret"
token_ttl_secs = 3600
admin_user = "admin"
admin_password = "adminpwd"
invite_code = "invitecode"

[server]
bind = "0.0.0.0:3000"
max_scan_concurrency = 1
max_hls_concurrency = 2
max_in_flight = 256
rate_limit_per_minute = 0
rate_limit_user_per_minute = 0
rate_limit_ip_per_minute = 0
rate_limit_allow_users = ["admin"]
rate_limit_allow_ips = ["127.0.0.1"]
rate_limit_block_users = []
rate_limit_block_ips = []
job_workers = 2
job_poll_interval_ms = 500
job_max_attempts = 3
job_retention_hours = 168
job_cleanup_interval_secs = 3600
job_running_timeout_secs = 3600

[bangumi]
access_token = ""
user_agent = "Anicargo/0.1"

[logging]
enabled = false
path = "~/.cache/anicargo/logs"
level = "info"
max_total_mb = 200

[qbittorrent]
base_url = "http://127.0.0.1:8080"
username = "admin"
password = "adminadmin"
download_dir = "/data/anime"
```

Required:
- `ANICARGO_MEDIA_DIR` (absolute or relative path to your video folder)
- `ANICARGO_DATABASE_URL` (PostgreSQL connection string)

Optional:
- `ANICARGO_CACHE_DIR` (default: `.cache`)
- `ANICARGO_FFMPEG_PATH` (default: `ffmpeg`)
- `ANICARGO_HLS_SEGMENT_SECS` (default: `6`)
- `ANICARGO_HLS_PLAYLIST_LEN` (default: `0`, keep all segments in playlist)
- `ANICARGO_HLS_LOCK_TIMEOUT_SECS` (default: `3600`, stale lock cleanup)
- `ANICARGO_HLS_TRANSCODE` (default: `false`, set `true` to force H.264/AAC)
- `ANICARGO_DB_MAX_CONNECTIONS` (default: `5`)
- `ANICARGO_BIND` (default: `0.0.0.0:3000`)
- `ANICARGO_MAX_SCAN_CONCURRENCY` (default: `1`)
- `ANICARGO_MAX_HLS_CONCURRENCY` (default: `2`)
- `ANICARGO_MAX_IN_FLIGHT` (default: `256`, `0` disables)
- `ANICARGO_RATE_LIMIT_PER_MINUTE` (default: `0`, `0` disables)
- `ANICARGO_RATE_LIMIT_USER_PER_MINUTE` (default: `0`, falls back to `ANICARGO_RATE_LIMIT_PER_MINUTE`)
- `ANICARGO_RATE_LIMIT_IP_PER_MINUTE` (default: `0`, falls back to `ANICARGO_RATE_LIMIT_PER_MINUTE`)
- `ANICARGO_RATE_LIMIT_ALLOW_USERS` (default: empty, CSV list)
- `ANICARGO_RATE_LIMIT_ALLOW_IPS` (default: empty, CSV list)
- `ANICARGO_RATE_LIMIT_BLOCK_USERS` (default: empty, CSV list)
- `ANICARGO_RATE_LIMIT_BLOCK_IPS` (default: empty, CSV list)
- `ANICARGO_JOB_WORKERS` (default: `2`)
- `ANICARGO_JOB_POLL_INTERVAL_MS` (default: `500`)
- `ANICARGO_JOB_MAX_ATTEMPTS` (default: `3`)
- `ANICARGO_JOB_RETENTION_HOURS` (default: `168`, `0` disables)
- `ANICARGO_JOB_CLEANUP_INTERVAL_SECS` (default: `3600`)
- `ANICARGO_JOB_RUNNING_TIMEOUT_SECS` (default: `3600`)

Rate limiting applies per user when a valid `Authorization: Bearer <token>` is present,
otherwise it falls back to per-IP. Block lists take priority over allow lists.
- `ANICARGO_ADMIN_USER` (default: `admin`)
- `ANICARGO_ADMIN_PASSWORD` (default: `adminpwd`)
- `ANICARGO_INVITE_CODE` (default: `invitecode`)
- `ANICARGO_JWT_SECRET` (default: `dev-secret`)
- `ANICARGO_TOKEN_TTL_SECS` (default: `3600`)

Bangumi (optional):
- `ANICARGO_BANGUMI_ACCESS_TOKEN` (default: empty)
- `ANICARGO_BANGUMI_USER_AGENT` (default: `Anicargo/0.1`)

Logging (optional, default off):
- `ANICARGO_LOG_ENABLED` (default: `false`)
- `ANICARGO_LOG_PATH` (default: `~/.cache/anicargo/logs`)
- `ANICARGO_LOG_LEVEL` (default: `info`)
- `ANICARGO_LOG_MAX_MB` (default: `200`)

qBittorrent (optional):
- `ANICARGO_QBITTORRENT_BASE_URL` (default: `http://127.0.0.1:8080`)
- `ANICARGO_QBITTORRENT_USERNAME`
- `ANICARGO_QBITTORRENT_PASSWORD`
- `ANICARGO_QBITTORRENT_DOWNLOAD_DIR`

Logs rotate daily; when the directory exceeds `ANICARGO_LOG_MAX_MB`, the oldest logs are removed.

Example:

```bash
export ANICARGO_MEDIA_DIR=/data/anime
export ANICARGO_CACHE_DIR=./.cache
export ANICARGO_DATABASE_URL=postgres://anicargo:anicargo@127.0.0.1:5432/anicargo
export ANICARGO_BIND=0.0.0.0:3000
```

## Run the API Server

```bash
cargo run -p anicargo-api
```

Endpoints:
- `GET /api/library` -> list media entries
- `GET /api/library?refresh=true` -> enqueue index (admin) and return cached entries
- `GET /api/stream/:id` -> returns HLS playlist URL, or 202 + job id if queued
- `POST /api/auth/login` -> returns JWT token
- `POST /api/users` -> create user with invite code
- `DELETE /api/users/:id` -> delete user (admin or self)
- `POST /api/matches/auto` -> enqueue auto matching (admin)
- `GET /api/matches/:id` -> current match for media id
- `POST /api/matches/:id` -> set manual match (admin)
- `DELETE /api/matches/:id` -> clear match (admin)
- `GET /api/matches/:id/candidates` -> list match candidates
- `POST /api/jobs/index` -> enqueue library index (admin)
- `POST /api/jobs/auto-match` -> enqueue auto match (admin, optional body)
- `POST /api/jobs/hls/:id` -> enqueue HLS generation
- `GET /api/jobs/:id` -> job status/result
- `GET /api/jobs/:id/stream` -> job status stream (SSE)
- `GET /hls/:token/:id/index.m3u8` -> HLS playlist (token in path)

## PostgreSQL via Docker

From `backend/docker/postgres`:

```bash
docker compose up -d
```

Connection string example:

```
postgres://anicargo:anicargo@127.0.0.1:5432/anicargo
```

## Auth Flow (Minimal)

Create a user (invite code required):

```bash
curl -X POST http://127.0.0.1:3000/api/users \\
  -H 'Content-Type: application/json' \\
  -d '{"user_id":"alice","password":"secret","invite_code":"invitecode"}'
```

Login and extract token:

```bash
curl -X POST http://127.0.0.1:3000/api/auth/login \\
  -H 'Content-Type: application/json' \\
  -d '{"user_id":"alice","password":"secret"}'
```

Use token for stream:

```bash
curl -H 'Authorization: Bearer <token>' \\
  http://127.0.0.1:3000/api/stream/<media-id>
```

Playback (token in path so segments inherit it):

```
mpv "http://127.0.0.1:3000/hls/<token>/<media-id>/index.m3u8"
```

## Run the CLI

```bash
cargo run -p anicargo-cli -- scan
cargo run -p anicargo-cli -- index    # scan + parse + store to database
cargo run -p anicargo-cli -- hls <media-id>
cargo run -p anicargo-cli -- bangumi-search <keyword>
cargo run -p anicargo-cli -- bangumi-sync <subject-id>
cargo run -p anicargo-cli -- qbittorrent-add <magnet>
cargo run -p anicargo-cli -- qbittorrent-sync
```

## Library Indexing

`anicargo-cli index` scans the media directory, parses filenames via Anitomy, skips unchanged files,
and removes missing entries:

`GET /api/library` reads from the database; run an index job (or CLI index) first.
- `media_files` -> file path/size/mtime
- `media_parses` -> parsed fields (title/episode/season/etc.)
- `bangumi_subjects` -> cached Bangumi subject metadata
- `bangumi_episodes` -> cached episode metadata

## Bangumi Cache

- `anicargo-cli bangumi-search <keyword>` searches for anime subjects.
- `anicargo-cli bangumi-sync <subject-id>` fetches subject + episodes and stores them in the database.

## Auto Matching

`POST /api/matches/auto` enqueues a background auto match job. It uses parsed titles to
search Bangumi, stores candidates in `match_candidates`, and writes auto matches to
`media_matches` when confidence exceeds the threshold.

Manual fixes can override auto matches via `POST /api/matches/:id` (admin only).

## Background Jobs

Heavy work (indexing, auto match, HLS generation) runs via the job queue.

- Enqueue: `POST /api/jobs/index`, `POST /api/jobs/auto-match`, `POST /api/jobs/hls/:id`
- Status: `GET /api/jobs/:id`
- Stream: `GET /api/jobs/:id/stream` (SSE)

Completed/failed jobs are automatically cleaned based on `job_retention_hours`. Running jobs
older than `job_running_timeout_secs` are re-queued or marked failed.

SSE events use the job status (`queued`, `running`, `retry`, `done`, `failed`) as the event name.

Example:

```bash
curl -N -H 'Authorization: Bearer <token>' \
  http://127.0.0.1:3000/api/jobs/<job-id>/stream
```

`POST /api/jobs/auto-match` accepts optional JSON:

```json
{
  "limit": 8,
  "min_candidate_score": 0.5,
  "min_confidence": 0.9
}
```

## qBittorrent Integration

- `qbittorrent-add` queues a magnet link via the WebUI API.
- `qbittorrent-sync` checks for completed torrents and triggers `index` + auto match.

## Workspace Layout

- `crates/anicargo-config` -> unified config loader + logging
- `crates/anicargo-media` -> media scanning + HLS pipeline
- `crates/anicargo-library` -> media index + filename parsing + metadata storage
- `crates/anicargo-bangumi` -> Bangumi API client
- `crates/anicargo-cli` -> command-line wrapper for media library
- `crates/anicargo-api` -> HTTP server (axum)
