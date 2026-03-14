-- Sync folders: local path <-> Drive folder
CREATE TABLE sync_folders (
    id               TEXT PRIMARY KEY NOT NULL,
    account_id       TEXT NOT NULL REFERENCES accounts(id),
    local_path       TEXT NOT NULL,
    drive_folder_id  TEXT NOT NULL,
    start_page_token TEXT,
    last_sync_at     INTEGER,
    paused           INTEGER NOT NULL DEFAULT 0
);
