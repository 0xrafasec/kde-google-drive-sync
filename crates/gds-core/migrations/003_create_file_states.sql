-- File states: known state per file in a sync folder
CREATE TABLE file_states (
    id              TEXT PRIMARY KEY NOT NULL,
    sync_folder_id  TEXT NOT NULL REFERENCES sync_folders(id),
    relative_path   TEXT NOT NULL,
    drive_file_id   TEXT,
    drive_md5       TEXT,
    drive_modified  INTEGER,
    local_md5       TEXT,
    local_modified  INTEGER,
    sync_state      TEXT NOT NULL,
    last_synced_at  INTEGER,
    UNIQUE(sync_folder_id, relative_path)
);

CREATE INDEX idx_file_states_folder ON file_states(sync_folder_id);
CREATE INDEX idx_file_states_drive_id ON file_states(drive_file_id);
