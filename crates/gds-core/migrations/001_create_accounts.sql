-- Accounts: linked Google account and keyring reference
CREATE TABLE accounts (
    id          TEXT PRIMARY KEY NOT NULL,
    email       TEXT NOT NULL,
    display_name TEXT,
    keyring_key TEXT NOT NULL,
    created_at  INTEGER NOT NULL
);
