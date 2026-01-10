# Anicargo Backend API

This document describes the HTTP API exposed by the Anicargo backend.

## Base URL

All endpoints are served under the server bind address, for example:

```
http://127.0.0.1:3000
```

## Auth

Most endpoints require a token. Provide it in one of two ways:

- `Authorization: Bearer <token>` header (preferred)
- `?token=<token>` query parameter

If the role level is too low, the API returns `404` ("not found") to keep
unauthorized resources hidden.

### Role levels

- 1: normal user
- 2: collector (can submit/view collection queue)
- 3: admin
- 4: admin (can assign level 3)
- 5: super admin

## Errors

All errors return JSON in the form:

```json
{"error":"message"}
```

Common status codes:

- 400: invalid input
- 401: missing/invalid token
- 404: not found / insufficient role level
- 409: conflict (dedup, already processed, etc.)
- 500: internal error

## Endpoints

### Auth and users

#### POST /api/auth/login

Request:

```json
{"user_id":"alice","password":"secret"}
```

Response:

```json
{
  "token":"<jwt>",
  "user_id":"alice",
  "role":"admin",
  "role_level":3,
  "expires_in":3600
}
```

#### POST /api/users

Create a user (invite code required).

Request:

```json
{"user_id":"alice","password":"secret","invite_code":"invitecode"}
```

Response:

```json
{"user_id":"alice","role":"user","role_level":1}
```

#### GET /api/users

Admin only. Returns user list.

Response:

```json
[
  {"user_id":"admin","role":"admin","role_level":5,"created_at":"..."},
  {"user_id":"alice","role":"user","role_level":1,"created_at":"..."}
]
```

#### DELETE /api/users/:id

Admin can delete any user; normal users can delete themselves.

#### PATCH /api/users/:id/role

Admin only. Cannot assign equal or higher role level, and cannot update self.

Request:

```json
{"role_level":3}
```

Response:

```json
{"user_id":"bob","role":"admin","role_level":3}
```

### Library and media

#### GET /api/library

Returns all indexed media entries.

Query:

- `refresh=true` (admin only) enqueue a background index job

Response:

```json
[
  {"id":"b2e6d6ac0e422641","filename":"spy_family.mp4","size":407464533}
]
```

#### GET /api/media/:id

Returns file entry, parse info, match info, and user progress.

Response:

```json
{
  "entry":{"id":"...","filename":"...","size":123},
  "parse":{"title":"Spy x Family","episode":"12","season":null,"year":"2025","release_group":"Sakurato","resolution":"1080p"},
  "matched":{
    "subject":{"id":329906,"name":"SPYxFAMILY","name_cn":"...","air_date":"2022-04-09","total_episodes":25},
    "episode":null,
    "method":"auto",
    "confidence":0.85,
    "reason":"title=0.85"
  },
  "progress":{"media_id":"...","position_secs":123.4,"duration_secs":1420.0}
}
```

#### GET /api/media/:id/next

Returns the next episode info for the matched subject, plus the next media file
if it exists in the library.

Response:

```json
{
  "subject":{"id":329906,"name":"SPYxFAMILY","name_cn":"...","air_date":"2022-04-09","total_episodes":25},
  "current_episode":{"id":101,"sort":12,"ep":12,"name":"...","name_cn":"...","air_date":"..."},
  "next_episode":{"id":102,"sort":13,"ep":13,"name":"...","name_cn":"...","air_date":"..."},
  "next_media":{"id":"...","filename":"...","size":123456}
}
```

#### GET /api/media/:id/episodes

Returns episodes for the matched subject (auto-syncs on first request).

#### GET /api/subjects/:id/episodes

Returns episodes for a specific subject (auto-syncs on first request).

Response (both endpoints):

```json
{
  "subject":{"id":329906,"name":"SPYxFAMILY","name_cn":"...","air_date":"2022-04-09","total_episodes":25},
  "episodes":[{"id":123,"sort":1.0,"ep":1.0,"name":"...","name_cn":"...","air_date":"2022-04-09"}]
}
```

### Streaming (HLS)

#### GET /api/stream/:id

If HLS is ready, returns the playlist URL. Otherwise enqueues a job.

Ready response:

```json
{"id":"...","playlist_url":"/hls/<token>/<id>/index.m3u8"}
```

Pending response:

```json
{"status":"queued","job_id":42}
```

#### GET /hls/:token/:id/:file
#### GET /hls/:id/:file

Serves HLS files. If the token is not in the path, pass it via header or
`?token=` query.

### Settings

#### GET /api/settings

Response:

```json
{"display_name":null,"theme":"default","playback_speed":1.0,"subtitle_lang":null}
```

#### PUT /api/settings

Request:

```json
{"display_name":"Alice","theme":"default","playback_speed":1.0,"subtitle_lang":"zh"}
```

Response: same as GET.

### Playback progress

#### GET /api/progress

Query:

- `limit` (default 50, max 200)
- `offset`

Response:

```json
{
  "items":[
    {"media_id":"...","filename":"...","position_secs":12.3,"duration_secs":1440.0,"updated_at":"..."}
  ]
}
```

#### GET /api/progress/:id

Response:

```json
{"media_id":"...","position_secs":12.3,"duration_secs":1440.0}
```

#### PUT /api/progress/:id

Request:

```json
{"position_secs":120.5,"duration_secs":1440.0}
```

Response: same as GET.

### Matching

#### POST /api/matches/auto

Admin only. Enqueue auto-match job.

Response:

```json
{"job_id":123}
```

#### GET /api/matches/:id

Response:

```json
{
  "current":{
    "media_id":"...",
    "subject_id":329906,
    "episode_id":null,
    "method":"auto",
    "confidence":0.85,
    "reason":"title=0.85"
  }
}
```

#### POST /api/matches/:id

Admin only. Set manual match.

Request:

```json
{"subject_id":329906,"episode_id":12}
```

Response: `204 No Content`

#### DELETE /api/matches/:id

Admin only. Clears manual/auto match.

#### GET /api/matches/:id/candidates

Response:

```json
{
  "candidates":[
    {"subject_id":329906,"confidence":1.0,"reason":"title=1.00","name":"SPYxFAMILY","name_cn":"..."}
  ]
}
```

### Jobs

#### POST /api/jobs/index

Admin only. Enqueue indexing job.

#### POST /api/jobs/auto-match

Admin only. Enqueue auto-match job with options:

```json
{"limit":100,"min_candidate_score":0.3,"min_confidence":0.3}
```

#### POST /api/jobs/hls/:id

Enqueue HLS generation for a media item.

#### GET /api/jobs/:id

Response:

```json
{
  "job":{
    "id":42,
    "job_type":"index",
    "status":"running",
    "attempts":0,
    "max_attempts":3,
    "result":null,
    "last_error":null
  }
}
```

#### GET /api/jobs/:id/stream

Server-sent events. Event name is the job status (`queued`, `running`,
`retry`, `done`, `failed`). Each event data is the JSON job status payload.

### Resource collection

#### GET /api/collection

Collector (level 2) and admin only. Admins see all; collectors see only
their own.

Query:

- `status=pending|approved|rejected`

Response:

```json
{
  "items":[
    {
      "id":1,
      "submitter_id":"alice",
      "kind":"magnet",
      "status":"pending",
      "magnet":"magnet:?xt=...",
      "torrent_name":null,
      "note":"season 3",
      "decision_note":null,
      "created_at":"...",
      "decided_at":null,
      "decided_by":null
    }
  ]
}
```

#### POST /api/collection/magnet

Collector (level 2) and admin only.

Request:

```json
{"magnet":"magnet:?xt=...","note":"optional note"}
```

Response:

```json
{"id":1,"status":"pending"}
```

#### POST /api/collection/torrent

Collector (level 2) and admin only. Multipart form upload.

Fields:

- `torrent` or `file`: the .torrent file (max 4 MB)
- `note` (optional)

Response:

```json
{"id":2,"status":"pending"}
```

#### POST /api/collection/:id/approve

Admin only. Submits the magnet/torrent to qBittorrent.

Optional JSON body:

```json
{"note":"approved for download"}
```

#### POST /api/collection/:id/reject

Admin only. Optional JSON body:

```json
{"note":"duplicate"}
```

#### DELETE /api/collection/:id

Admin can delete any submission. Submitters can delete only when status is
`pending`.

Notes:

- Duplicate magnets/torrents (hash-based) return `409`.

### qBittorrent

#### POST /api/qbittorrent/magnet

Admin only.

Request:

```json
{"magnet":"magnet:?xt=..."}
```

Response:

```json
{"status":"queued"}
```

#### POST /api/qbittorrent/torrent

Admin only. Multipart form upload.

Fields:

- `torrent` or `file`: the .torrent file (max 4 MB)

Response:

```json
{"status":"queued"}
```

#### GET /api/admin/qbittorrent/completed

Admin only.

Response:

```json
[
  {
    "hash":"...",
    "name":"...",
    "state":"completed",
    "progress":1.0,
    "save_path":"/downloads",
    "content_path":"/downloads/...",
    "completion_on":1710000000
  }
]
```

### Admin

#### GET /api/admin/metrics

Admin only. System, storage, network, and qBittorrent metrics.

Response:

```json
{
  "uptime_secs":3600,
  "media_count":120,
  "media_total_bytes":1234567890,
  "job_counts":{"queued":0,"running":1,"retry":0,"done":8,"failed":0},
  "system":{"total_memory_bytes":0,"used_memory_bytes":0,"process_memory_bytes":0,"cpu_usage_percent":2.1},
  "storage":{
    "media_dir":{"mount_point":"/","total_bytes":0,"available_bytes":0},
    "cache_dir":{"mount_point":"/","total_bytes":0,"available_bytes":0},
    "qbittorrent_download_dir":null
  },
  "network":{
    "rx_bytes":0,
    "tx_bytes":0,
    "rx_bytes_per_sec":0.0,
    "tx_bytes_per_sec":0.0,
    "interfaces":[{"name":"eth0","rx_bytes":0,"tx_bytes":0}]
  },
  "in_flight_requests":0,
  "max_in_flight":256,
  "qbittorrent":{
    "download_speed_bytes":0,
    "upload_speed_bytes":0,
    "download_total_bytes":0,
    "upload_total_bytes":0,
    "download_rate_limit":-1,
    "upload_rate_limit":-1,
    "dht_nodes":0,
    "connection_status":"connected"
  }
}
```

#### GET /api/admin/jobs

Admin only. Job queue list.

Query:

- `status=queued|running|retry|done|failed`
- `limit` (default 100, max 500)
- `offset`

Response:

```json
{"jobs":[{"id":1,"job_type":"index","status":"done","attempts":0,"max_attempts":3,"payload":{},"result":null,"last_error":null,"scheduled_at":"...","locked_at":null,"locked_by":null,"created_at":"...","updated_at":"..."}]}
```
