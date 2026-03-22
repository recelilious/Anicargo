CREATE TABLE yuc_catalogs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    catalog_key TEXT NOT NULL UNIQUE,
    catalog_kind TEXT NOT NULL,
    season_year INTEGER,
    season_month INTEGER,
    title TEXT NOT NULL,
    source_url TEXT NOT NULL,
    source_hash TEXT NOT NULL,
    fetched_at TEXT NOT NULL,
    refreshed_at TEXT NOT NULL
);

CREATE TABLE yuc_catalog_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    yuc_catalog_id INTEGER NOT NULL REFERENCES yuc_catalogs(id) ON DELETE CASCADE,
    sort_index INTEGER NOT NULL,
    weekday_id INTEGER NOT NULL,
    weekday_cn TEXT NOT NULL,
    weekday_en TEXT NOT NULL,
    weekday_ja TEXT NOT NULL,
    title TEXT NOT NULL,
    title_cn TEXT NOT NULL,
    title_original TEXT,
    broadcast_time TEXT,
    broadcast_label TEXT,
    episode_note TEXT,
    platform_note TEXT,
    source_image_url TEXT,
    bangumi_subject_id INTEGER,
    bangumi_match_score REAL,
    bangumi_match_title TEXT,
    bangumi_matched_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_yuc_catalog_entries_catalog_sort
    ON yuc_catalog_entries (yuc_catalog_id, sort_index);

CREATE INDEX idx_yuc_catalog_entries_catalog_weekday
    ON yuc_catalog_entries (yuc_catalog_id, weekday_id, sort_index);

CREATE INDEX idx_yuc_catalog_entries_subject
    ON yuc_catalog_entries (bangumi_subject_id);

CREATE TABLE bangumi_subject_cache (
    bangumi_subject_id INTEGER PRIMARY KEY,
    title TEXT NOT NULL,
    title_cn TEXT NOT NULL,
    summary TEXT NOT NULL,
    air_date TEXT,
    air_weekday INTEGER,
    total_episodes INTEGER,
    image_portrait TEXT,
    image_banner TEXT,
    tags_json TEXT NOT NULL DEFAULT '[]',
    rating_score REAL,
    release_status TEXT NOT NULL,
    metadata_refreshed_at TEXT NOT NULL,
    status_refreshed_at TEXT NOT NULL
);

CREATE INDEX idx_bangumi_subject_cache_status_refreshed
    ON bangumi_subject_cache (status_refreshed_at);
