CREATE TABLE IF NOT EXISTS playback_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    viewer_kind TEXT NOT NULL,
    viewer_key TEXT NOT NULL,
    bangumi_subject_id INTEGER NOT NULL,
    bangumi_episode_id INTEGER NOT NULL,
    media_inventory_id INTEGER,
    last_played_at TEXT NOT NULL,
    created_at TEXT NOT NULL,
    play_count INTEGER NOT NULL DEFAULT 1,
    UNIQUE(viewer_kind, viewer_key, bangumi_episode_id),
    FOREIGN KEY(media_inventory_id) REFERENCES media_inventory(id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_playback_history_viewer_last_played
    ON playback_history(viewer_kind, viewer_key, last_played_at DESC);

