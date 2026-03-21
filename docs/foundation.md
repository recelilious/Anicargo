# Anicargo Foundation Notes

Updated: 2026-03-22

## Confirmed Decisions

- Metadata source: Bangumi API
- Resource discovery:
  - airing anime: prefer AnimeGarden RSS polling
  - backfill or manual repair: use targeted AnimeGarden API search
- Database: SQLite
- Backend language: Rust
- Web frontend: React + TypeScript

## Why SQLite

SQLite is the best fit for the current target shape:

- no separate database service to deploy
- not as fragile as JSON files
- supports transactions, indexes, foreign keys, and migrations
- easy to bundle into a private single-server deployment

Recommended first setup:

- one app database file
- enable WAL mode
- keep media files and database separate

Suggested split:

- database: app state, subscriptions, jobs, media index
- filesystem: downloaded videos, subtitles, posters cache, temp files

## Bangumi IDs

Bangumi has stable numeric identifiers that are suitable as external keys.

Recommended fields:

- `bangumi_subject_id`: unique key for one anime entry / season / OVA / movie
- `bangumi_episode_id`: unique key for one episode entry

Important note:

- `subject.id` is usually the right unit for download and playback management
- it represents a single Bangumi subject, not an entire franchise

Recommended database rule:

- use internal app `id` as primary key
- add `bangumi_subject_id` as a unique indexed external key

This gives us flexibility if we later add more metadata providers.

## Update Tracking Strategy

Bangumi is reliable for:

- subject identity
- season calendar
- date / weekday level update information
- episode lists

Bangumi should not be the only timing source for the actual downloader scheduler.

Recommended logic:

1. Use Bangumi to decide whether a subject is airing and which weekday it updates on.
2. For airing anime, subscribe to one or a few aggregated AnimeGarden RSS feeds.
3. Match incoming RSS items to `bangumi_subject_id` locally.
4. If matching fails or a backfill is requested, run targeted AnimeGarden API searches.

This avoids calling search APIs repeatedly for every subscribed title.

## Downloader Direction

Current principle:

- do not require users to manage qBittorrent manually
- keep the downloader behind an internal `DownloadEngine` abstraction

Short-term engineering direction:

- support an embeddable downloader path
- keep a compatibility layer so we can swap implementations later

## Suggested First Tables

- `anime`
- `anime_alias`
- `episode`
- `user`
- `subscription`
- `availability_policy`
- `download_job`
- `download_file`
- `library_asset`
- `notification_event`

## Minimal `anime` Fields

- internal `id`
- `bangumi_subject_id`
- `title`
- `title_cn`
- `type`
- `status`
- `air_date`
- `air_weekday`
- `total_episodes`
- `is_airing`
- `cover_image`
- `updated_at`

## Minimal `subscription` Fields

- internal `id`
- `user_id`
- `anime_id`
- `created_at`
- `mode`

Where `mode` can start with:

- `normal`
- `admin_forced`

## Minimal `download_job` Fields

- internal `id`
- `anime_id`
- `job_type`
- `status`
- `source`
- `magnet`
- `started_at`
- `finished_at`
- `error_message`

## Next Practical Step

Build v0 around these three capabilities:

1. sync Bangumi subject and episode data into SQLite
2. create subscriptions and threshold rules
3. consume AnimeGarden feed items and map them onto subscribed subjects
