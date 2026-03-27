# Anicargo API Map

This file is a repository-level API overview. Detailed endpoint references live in subproject docs.

## 1. HTTP Surfaces

Anicargo currently exposes two HTTP surfaces:

### Main backend API

- Default base URL: `http://127.0.0.1:4000`
- Detailed reference: [backend/docs/API.md](../backend/docs/API.md)

Main route groups:

- `/api/health`
- `/api/public/*`
- `/api/auth/*`
- `/api/admin/*`

### Downloader service API

- Default standalone base URL: `http://127.0.0.1:4010`
- Detailed reference: [services/downloader/docs/API.md](../services/downloader/docs/API.md)

This API is optional when the downloader is embedded inside the backend. It is mainly used for isolated testing and service debugging.

## 2. Identity Model

The backend distinguishes three viewer modes:

- Guest device viewer: identified by `x-anicargo-device-id`
- Logged-in user: guest device plus `Authorization: Bearer <token>`
- Admin session: `x-anicargo-admin-token`

The user-facing web flow is guest-first. User accounts are optional. Admin access is separate and does not reuse the standard user login route.

## 3. Primary User Flows

### Season browsing

`GET /api/public/calendar`

Returns the current season schedule cached from Yuc and enriched with Bangumi metadata/status.

### Search

`GET /api/public/search`

Searches Bangumi-backed subjects with filters and pagination.

### Subject detail and playback

- `GET /api/public/subjects/{subject_id}`
- `GET /api/public/subjects/{subject_id}/episodes/{episode_id}/playback`
- `GET /api/public/media/{media_id}/stream`

### Subscription

`POST /api/public/subscriptions/{subject_id}/toggle`

Subscription changes feed the backend download-demand pipeline.

### Resource library and history

- `GET /api/public/resources`
- `GET /api/public/downloads/active`
- `GET /api/public/history`
- `POST /api/public/history/playback`

## 4. Admin Flows

Admin routes manage:

- policy
- fansub rules
- runtime overview
- download jobs
- candidate inspection
- execution inspection
- manual force triggers

See [backend/docs/API.md](../backend/docs/API.md) for the route-level details.

## 5. Downloader Service Flows

The standalone downloader API covers:

- runtime settings
- metadata inspection
- task creation
- task listing
- task update
- pause/resume/delete
- download and seed queue snapshots

See [services/downloader/docs/API.md](../services/downloader/docs/API.md) for the detailed contract.
