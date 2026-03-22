ALTER TABLE download_jobs ADD COLUMN selected_candidate_id INTEGER;
ALTER TABLE download_jobs ADD COLUMN selection_updated_at TEXT;
ALTER TABLE download_jobs ADD COLUMN last_search_run_id INTEGER;
ALTER TABLE download_jobs ADD COLUMN search_status TEXT NOT NULL DEFAULT 'idle';

CREATE TABLE IF NOT EXISTS resource_search_runs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    download_job_id INTEGER NOT NULL,
    bangumi_subject_id INTEGER NOT NULL,
    strategy TEXT NOT NULL,
    status TEXT NOT NULL,
    candidate_count INTEGER NOT NULL DEFAULT 0,
    best_candidate_id INTEGER,
    notes TEXT,
    created_at TEXT NOT NULL,
    completed_at TEXT
);

CREATE TABLE IF NOT EXISTS resource_candidates (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    download_job_id INTEGER NOT NULL,
    search_run_id INTEGER NOT NULL,
    bangumi_subject_id INTEGER NOT NULL,
    provider TEXT NOT NULL,
    provider_resource_id TEXT NOT NULL,
    title TEXT NOT NULL,
    href TEXT NOT NULL,
    magnet TEXT NOT NULL,
    release_type TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    fansub_name TEXT,
    publisher_name TEXT NOT NULL,
    source_created_at TEXT NOT NULL,
    source_fetched_at TEXT NOT NULL,
    resolution TEXT,
    locale_hint TEXT,
    is_raw INTEGER NOT NULL DEFAULT 0,
    score REAL NOT NULL,
    rejected_reason TEXT,
    discovered_at TEXT NOT NULL,
    UNIQUE(download_job_id, provider, provider_resource_id)
);

CREATE INDEX IF NOT EXISTS idx_resource_search_runs_job
ON resource_search_runs (download_job_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_resource_candidates_job
ON resource_candidates (download_job_id, score DESC, discovered_at DESC);
