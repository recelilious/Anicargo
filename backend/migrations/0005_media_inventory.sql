ALTER TABLE resource_candidates ADD COLUMN slot_key TEXT NOT NULL DEFAULT '';
ALTER TABLE resource_candidates ADD COLUMN episode_index REAL;
ALTER TABLE resource_candidates ADD COLUMN episode_end_index REAL;
ALTER TABLE resource_candidates ADD COLUMN is_collection INTEGER NOT NULL DEFAULT 0;

ALTER TABLE download_executions ADD COLUMN slot_key TEXT NOT NULL DEFAULT 'primary';
ALTER TABLE download_executions ADD COLUMN episode_index REAL;
ALTER TABLE download_executions ADD COLUMN episode_end_index REAL;
ALTER TABLE download_executions ADD COLUMN is_collection INTEGER NOT NULL DEFAULT 0;
ALTER TABLE download_executions ADD COLUMN last_indexed_at TEXT;

CREATE TABLE IF NOT EXISTS media_inventory (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    bangumi_subject_id INTEGER NOT NULL,
    download_job_id INTEGER NOT NULL,
    download_execution_id INTEGER NOT NULL,
    resource_candidate_id INTEGER NOT NULL,
    slot_key TEXT NOT NULL,
    relative_path TEXT NOT NULL,
    absolute_path TEXT NOT NULL,
    file_name TEXT NOT NULL,
    file_ext TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    episode_index REAL,
    episode_end_index REAL,
    is_collection INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(download_execution_id, relative_path)
);

CREATE INDEX IF NOT EXISTS idx_media_inventory_subject
ON media_inventory (bangumi_subject_id, status, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_media_inventory_execution
ON media_inventory (download_execution_id, updated_at DESC);
