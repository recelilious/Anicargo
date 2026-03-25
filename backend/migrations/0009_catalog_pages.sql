ALTER TABLE yuc_catalog_entries ADD COLUMN group_key TEXT;
ALTER TABLE yuc_catalog_entries ADD COLUMN group_title TEXT;
ALTER TABLE yuc_catalog_entries ADD COLUMN catalog_label TEXT;
ALTER TABLE yuc_catalog_entries ADD COLUMN entry_release_status TEXT;

CREATE INDEX idx_yuc_catalog_entries_catalog_group
    ON yuc_catalog_entries (yuc_catalog_id, group_key, sort_index);
