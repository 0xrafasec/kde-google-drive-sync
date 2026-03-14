-- Single-row table: OAuth app client_id and client_secret for the daemon.
-- Stored in DB so the daemon can read them without keyring (reliable when running in background).
-- Rely on state.db file permissions (e.g. 600); do not commit the DB.
CREATE TABLE oauth_app_credentials (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    client_id TEXT NOT NULL,
    client_secret TEXT NOT NULL
);
