# Anicargo Backend

This backend is split into a media library, a CLI, and an API server. The media library owns scanning and HLS output; the API only exposes HTTP endpoints.

## Prerequisites

- Rust (stable)
- ffmpeg (available in PATH, or set `ANICARGO_FFMPEG_PATH`)

## Configuration

Required:
- `ANICARGO_MEDIA_DIR` (absolute or relative path to your video folder)
- `ANICARGO_DATABASE_URL` (PostgreSQL connection string)

Optional:
- `ANICARGO_CACHE_DIR` (default: `.cache`)
- `ANICARGO_FFMPEG_PATH` (default: `ffmpeg`)
- `ANICARGO_HLS_SEGMENT_SECS` (default: `6`)
- `ANICARGO_HLS_PLAYLIST_LEN` (default: `0`, keep all segments in playlist)
- `ANICARGO_HLS_TRANSCODE` (default: `false`, set `true` to force H.264/AAC)
- `ANICARGO_BIND` (default: `0.0.0.0:3000`)
- `ANICARGO_ADMIN_USER` (default: `admin`)
- `ANICARGO_ADMIN_PASSWORD` (default: `adminpwd`)
- `ANICARGO_INVITE_CODE` (default: `invitecode`)
- `ANICARGO_JWT_SECRET` (default: `dev-secret`)
- `ANICARGO_TOKEN_TTL_SECS` (default: `3600`)

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
- `GET /api/stream/:id` -> returns HLS playlist URL (requires token)
- `POST /api/auth/login` -> returns JWT token
- `POST /api/users` -> create user with invite code
- `DELETE /api/users/:id` -> delete user (admin or self)
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
cargo run -p anicargo-cli -- hls <media-id>
```

## Workspace Layout

- `crates/anicargo-media` -> media scanning + HLS pipeline
- `crates/anicargo-cli` -> command-line wrapper for media library
- `crates/anicargo-api` -> HTTP server (axum)
