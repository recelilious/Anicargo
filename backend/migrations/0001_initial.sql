CREATE TABLE IF NOT EXISTS devices (
    id TEXT PRIMARY KEY,
    created_at TEXT NOT NULL,
    last_seen_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS admin_accounts (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS user_sessions (
    token TEXT PRIMARY KEY,
    user_id INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS admin_sessions (
    token TEXT PRIMARY KEY,
    admin_id INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    FOREIGN KEY(admin_id) REFERENCES admin_accounts(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS device_subscriptions (
    device_id TEXT NOT NULL,
    bangumi_subject_id INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY(device_id, bangumi_subject_id),
    FOREIGN KEY(device_id) REFERENCES devices(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS user_subscriptions (
    user_id INTEGER NOT NULL,
    bangumi_subject_id INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY(user_id, bangumi_subject_id),
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS fansub_rules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    fansub_name TEXT NOT NULL,
    locale_preference TEXT NOT NULL,
    priority INTEGER NOT NULL,
    is_blacklist INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS download_policies (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    subscription_threshold INTEGER NOT NULL,
    replacement_window_hours INTEGER NOT NULL,
    prefer_same_fansub INTEGER NOT NULL,
    updated_at TEXT NOT NULL
);

INSERT INTO download_policies (
    id,
    subscription_threshold,
    replacement_window_hours,
    prefer_same_fansub,
    updated_at
)
SELECT
    1,
    2,
    72,
    1,
    CURRENT_TIMESTAMP
WHERE NOT EXISTS (
    SELECT 1 FROM download_policies WHERE id = 1
);
