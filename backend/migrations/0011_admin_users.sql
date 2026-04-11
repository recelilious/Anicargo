ALTER TABLE users ADD COLUMN is_admin INTEGER NOT NULL DEFAULT 0;

UPDATE users
SET is_admin = 1
WHERE username IN (
    SELECT username FROM admin_accounts
);

INSERT INTO users (username, password_hash, created_at, is_admin)
SELECT
    admin_accounts.username,
    admin_accounts.password_hash,
    admin_accounts.created_at,
    1
FROM admin_accounts
WHERE NOT EXISTS (
    SELECT 1
    FROM users
    WHERE users.username = admin_accounts.username
);
