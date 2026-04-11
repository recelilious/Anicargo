# Backend API Reference

Base URL:

- `http://127.0.0.1:4000`

All JSON responses are wrapped as:

```json
{
  "data": {}
}
```

## 1. Headers

### Guest or device viewer

- `x-anicargo-device-id: <stable-device-id>`

### Logged-in user

- `Authorization: Bearer <user-token>`

### Admin

- Preferred: `Authorization: Bearer <user-token>` for an account with `is_admin = true`
- Legacy compatibility: `x-anicargo-admin-token: <admin-token>`

## 2. Public Routes

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/api/health` | Health probe |
| GET | `/api/public/bootstrap` | Guest/user bootstrap payload |
| GET | `/api/public/calendar` | Current season calendar |
| GET | `/api/public/catalogs/manifest` | Optional Yuc catalog availability |
| GET | `/api/public/catalogs/{kind}` | `preview` or `special` catalog page |
| GET | `/api/public/search` | Bangumi-backed subject search |
| GET | `/api/public/subscriptions` | Current viewer subscription list |
| GET | `/api/public/history` | Current viewer playback history |
| GET | `/api/public/resources` | Indexed resource library |
| GET | `/api/public/downloads/active` | Active download summary |
| GET | `/api/public/subjects/{subject_id}` | Subject detail + episodes + subscription state |
| GET | `/api/public/subjects/{subject_id}/download-status` | Subject-level download state |
| GET | `/api/public/subjects/{subject_id}/episodes/{episode_id}/playback` | Playback readiness for one episode |
| GET | `/api/public/media/{media_id}/stream` | Byte-range media streaming |
| POST | `/api/public/subscriptions/{subject_id}/toggle` | Subscribe or unsubscribe |
| POST | `/api/public/history/playback` | Record playback activity |

### Calendar Query Parameters

- `timezone`
- `deepNightMode`

### Search Query Parameters

Search supports the filters currently wired in the web client and backend:

- `keyword`
- `page`
- `page_size`
- `sort`
- repeated `tag`
- repeated `meta_tag`
- `air_date_start`
- `air_date_end`
- `rating_min`
- `rating_max`
- `rating_count_min`
- `rating_count_max`
- `rank_min`
- `rank_max`
- `nsfw_mode`

## 3. User Auth Routes

| Method | Path | Purpose |
| --- | --- | --- |
| POST | `/api/auth/register` | Create a user account |
| POST | `/api/auth/login` | User login |
| GET | `/api/auth/me` | Current user session |
| POST | `/api/auth/logout` | End user session |

## 4. Admin Routes

| Method | Path | Purpose |
| --- | --- | --- |
| POST | `/api/admin/login` | Compatibility login that accepts a normal user credential pair and succeeds only for an admin-capable account |
| POST | `/api/admin/logout` | End the current admin-capable session |
| GET | `/api/admin/dashboard` | Counts, policy, fansub rules |
| GET | `/api/admin/runtime` | Runtime telemetry snapshot |
| GET | `/api/admin/downloads` | Download jobs |
| GET | `/api/admin/downloads/{job_id}/candidates` | Candidate list for a job |
| GET | `/api/admin/downloads/{job_id}/executions` | Execution instances for a job |
| GET | `/api/admin/executions/{execution_id}/events` | Execution event log |
| POST | `/api/admin/downloads/{job_id}/execute` | Manual activation of a selected job |
| POST | `/api/admin/downloads/{subject_id}/force` | Force a subject into the pipeline |
| PUT | `/api/admin/policy` | Update policy values |
| POST | `/api/admin/fansub-rules` | Add or update fansub policy rules |

Admin routes are intended to be called with the same login session used by normal user pages. The web client no longer uses a separate admin-only sign-in flow.

## 5. Playback Notes

The backend currently serves original files through HTTP Range streaming. It does not generate HLS or DASH segments yet.

Implications:

- No re-encoding by default
- Source quality is preserved
- Browser compatibility depends on the original media container and codec support
