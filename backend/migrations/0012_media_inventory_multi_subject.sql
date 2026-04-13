CREATE TABLE IF NOT EXISTS media_inventory_v2 (
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
    UNIQUE(download_execution_id, bangumi_subject_id, slot_key, relative_path)
);

INSERT INTO media_inventory_v2 (
    id,
    bangumi_subject_id,
    download_job_id,
    download_execution_id,
    resource_candidate_id,
    slot_key,
    relative_path,
    absolute_path,
    file_name,
    file_ext,
    size_bytes,
    episode_index,
    episode_end_index,
    is_collection,
    status,
    created_at,
    updated_at
)
SELECT
    id,
    bangumi_subject_id,
    download_job_id,
    download_execution_id,
    resource_candidate_id,
    slot_key,
    relative_path,
    absolute_path,
    file_name,
    file_ext,
    size_bytes,
    episode_index,
    episode_end_index,
    is_collection,
    status,
    created_at,
    updated_at
FROM media_inventory;

DROP TABLE media_inventory;

ALTER TABLE media_inventory_v2 RENAME TO media_inventory;

CREATE INDEX IF NOT EXISTS idx_media_inventory_subject
ON media_inventory (bangumi_subject_id, status, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_media_inventory_execution
ON media_inventory (download_execution_id, updated_at DESC);
