-- Sync error log: per-file errors for retry and display
CREATE TABLE sync_errors (
    id             TEXT PRIMARY KEY NOT NULL,
    file_state_id  TEXT REFERENCES file_states(id),
    error_message  TEXT NOT NULL,
    occurred_at    INTEGER NOT NULL,
    retry_count    INTEGER NOT NULL DEFAULT 0
);
