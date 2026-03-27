# Downloader API Reference

Base URL:

- `http://127.0.0.1:4010`

## 1. Routes

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/api/health` | Health probe |
| GET | `/api/v1/runtime` | Runtime settings and queue overview |
| PATCH | `/api/v1/settings` | Update global runtime settings |
| POST | `/api/v1/inspect` | Read torrent metadata without creating a task |
| GET | `/api/v1/tasks` | List all tasks |
| POST | `/api/v1/tasks` | Create a task |
| GET | `/api/v1/tasks/{task_id}` | Fetch one task |
| PATCH | `/api/v1/tasks/{task_id}` | Update a task |
| POST | `/api/v1/tasks/{task_id}/pause` | Pause a task |
| POST | `/api/v1/tasks/{task_id}/resume` | Resume a task |
| DELETE | `/api/v1/tasks/{task_id}` | Delete a task |
| GET | `/api/v1/downloads` | List download-side tasks |
| GET | `/api/v1/seeds` | List seed-side tasks |

## 2. Task Source Types

Task creation and inspection currently support:

- magnet link
- `.torrent` bytes payload

The service first tries to obtain enough metadata to derive an `info_hash`, so it can deduplicate repeated create requests.

## 3. Task Scheduling Model

The scheduler considers:

- task category: download or seed
- paused/deleted/finished state
- numeric priority where `0` is highest
- creation time as the tiebreaker inside the same priority
- global limits
- per-task manual limits
- queue timeouts

## 4. Updateable Runtime Settings

Runtime settings include:

- maximum concurrent downloads
- maximum concurrent seeds
- global download limit
- global upload limit
- priority decay
- stall timeout
- total timeout
- scheduler interval

These values can be changed without restarting the service.

## 5. Common Response Use

Most control routes are intended for service-to-service or test-tool usage rather than direct user UI calls.

Typical flow:

1. `POST /api/v1/inspect`
2. `POST /api/v1/tasks`
3. `GET /api/v1/runtime`
4. `GET /api/v1/downloads`
5. `PATCH /api/v1/tasks/{task_id}` or pause/resume/delete as needed
