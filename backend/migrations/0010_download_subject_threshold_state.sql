ALTER TABLE download_subjects ADD COLUMN threshold_reached_once INTEGER NOT NULL DEFAULT 0;
ALTER TABLE download_subjects ADD COLUMN threshold_reached_at TEXT;
