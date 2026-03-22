CREATE TABLE IF NOT EXISTS download_subjects (
    bangumi_subject_id INTEGER PRIMARY KEY,
    release_status TEXT NOT NULL,
    demand_state TEXT NOT NULL,
    subscription_count INTEGER NOT NULL,
    threshold_snapshot INTEGER NOT NULL,
    last_queued_job_id INTEGER,
    last_triggered_at TEXT,
    last_evaluated_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS download_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    bangumi_subject_id INTEGER NOT NULL,
    trigger_kind TEXT NOT NULL,
    requested_by TEXT NOT NULL,
    release_status TEXT NOT NULL,
    season_mode TEXT NOT NULL,
    lifecycle TEXT NOT NULL,
    subscription_count INTEGER NOT NULL,
    threshold_snapshot INTEGER NOT NULL,
    engine_name TEXT NOT NULL,
    engine_job_ref TEXT,
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_download_jobs_subject_lifecycle
ON download_jobs (bangumi_subject_id, lifecycle, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_download_jobs_created_at
ON download_jobs (created_at DESC);
