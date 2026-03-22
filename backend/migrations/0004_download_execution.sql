CREATE TABLE IF NOT EXISTS download_executions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    download_job_id INTEGER NOT NULL,
    resource_candidate_id INTEGER NOT NULL,
    bangumi_subject_id INTEGER NOT NULL,
    engine_name TEXT NOT NULL,
    engine_execution_ref TEXT,
    execution_role TEXT NOT NULL,
    state TEXT NOT NULL,
    target_path TEXT NOT NULL,
    source_title TEXT NOT NULL,
    source_magnet TEXT NOT NULL,
    source_size_bytes INTEGER NOT NULL,
    source_fansub_name TEXT,
    downloaded_bytes INTEGER NOT NULL DEFAULT 0,
    uploaded_bytes INTEGER NOT NULL DEFAULT 0,
    download_rate_bytes INTEGER NOT NULL DEFAULT 0,
    upload_rate_bytes INTEGER NOT NULL DEFAULT 0,
    peer_count INTEGER NOT NULL DEFAULT 0,
    notes TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    started_at TEXT,
    completed_at TEXT,
    replaced_at TEXT,
    failed_at TEXT
);

CREATE TABLE IF NOT EXISTS download_execution_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    download_execution_id INTEGER NOT NULL,
    level TEXT NOT NULL,
    event_kind TEXT NOT NULL,
    message TEXT NOT NULL,
    downloaded_bytes INTEGER,
    uploaded_bytes INTEGER,
    download_rate_bytes INTEGER,
    upload_rate_bytes INTEGER,
    peer_count INTEGER,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_download_executions_job
ON download_executions (download_job_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_download_executions_subject_state
ON download_executions (bangumi_subject_id, state, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_download_execution_events_execution
ON download_execution_events (download_execution_id, created_at DESC);
