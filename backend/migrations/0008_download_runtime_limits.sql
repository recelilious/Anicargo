ALTER TABLE download_policies
ADD COLUMN max_concurrent_downloads INTEGER NOT NULL DEFAULT 5;

ALTER TABLE download_policies
ADD COLUMN upload_limit_mb INTEGER NOT NULL DEFAULT 0;

ALTER TABLE download_policies
ADD COLUMN download_limit_mb INTEGER NOT NULL DEFAULT 5;
