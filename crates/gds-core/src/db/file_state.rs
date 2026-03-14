//! File state repository.

use chrono::{DateTime, Utc};

use sqlx::SqlitePool;

use crate::model::{FileState, SyncState, SyncStateKind};

/// File state persistence.
pub struct FileStateRepository;

fn sync_state_to_json(s: &SyncState) -> String {
    serde_json::to_string(s).expect("SyncState serialization is infallible")
}

fn sync_state_from_json(s: &str) -> Result<SyncState, serde_json::Error> {
    serde_json::from_str(s)
}

impl FileStateRepository {
    /// Upsert a file state (insert or replace by sync_folder_id + relative_path).
    pub async fn upsert(pool: &SqlitePool, state: &FileState) -> Result<(), sqlx::Error> {
        let drive_modified = state.drive_modified.map(|t| t.timestamp_millis());
        let local_modified = state.local_modified.map(|t| t.timestamp_millis());
        let last_synced_at = state.last_synced_at.map(|t| t.timestamp());
        let sync_state_json = sync_state_to_json(&state.sync_state);

        sqlx::query(
            r#"
            INSERT INTO file_states (id, sync_folder_id, relative_path, drive_file_id, drive_md5, drive_modified, local_md5, local_modified, sync_state, last_synced_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(sync_folder_id, relative_path) DO UPDATE SET
                id = excluded.id,
                drive_file_id = excluded.drive_file_id,
                drive_md5 = excluded.drive_md5,
                drive_modified = excluded.drive_modified,
                local_md5 = excluded.local_md5,
                local_modified = excluded.local_modified,
                sync_state = excluded.sync_state,
                last_synced_at = excluded.last_synced_at
            "#,
        )
        .bind(&state.id)
        .bind(&state.sync_folder_id)
        .bind(&state.relative_path)
        .bind(&state.drive_file_id)
        .bind(&state.drive_md5)
        .bind(drive_modified)
        .bind(&state.local_md5)
        .bind(local_modified)
        .bind(&sync_state_json)
        .bind(last_synced_at)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Get file state by sync folder and relative path.
    pub async fn get_by_path(
        pool: &SqlitePool,
        sync_folder_id: &str,
        relative_path: &str,
    ) -> Result<Option<FileState>, sqlx::Error> {
        let row = sqlx::query_as::<_, (
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<String>,
            Option<i64>,
            String,
            Option<i64>,
        )>(
            "SELECT id, sync_folder_id, relative_path, drive_file_id, drive_md5, drive_modified, local_md5, local_modified, sync_state, last_synced_at FROM file_states WHERE sync_folder_id = ? AND relative_path = ?",
        )
        .bind(sync_folder_id)
        .bind(relative_path)
        .fetch_optional(pool)
        .await?;

        row.map(|r| row_to_file_state(r))
            .transpose()
            .map_err(|e| sqlx::Error::Decode(Box::new(e)))
    }

    /// Get file state by sync folder and drive file id.
    pub async fn get_by_drive_id(
        pool: &SqlitePool,
        sync_folder_id: &str,
        drive_file_id: &str,
    ) -> Result<Option<FileState>, sqlx::Error> {
        let row = sqlx::query_as::<_, (
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<String>,
            Option<i64>,
            String,
            Option<i64>,
        )>(
            "SELECT id, sync_folder_id, relative_path, drive_file_id, drive_md5, drive_modified, local_md5, local_modified, sync_state, last_synced_at FROM file_states WHERE sync_folder_id = ? AND drive_file_id = ?",
        )
        .bind(sync_folder_id)
        .bind(drive_file_id)
        .fetch_optional(pool)
        .await?;

        row.map(|r| row_to_file_state(r))
            .transpose()
            .map_err(|e| sqlx::Error::Decode(Box::new(e)))
    }

    /// List file states for a sync folder.
    pub async fn list_by_folder(
        pool: &SqlitePool,
        sync_folder_id: &str,
    ) -> Result<Vec<FileState>, sqlx::Error> {
        let rows = sqlx::query_as::<_, (
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<String>,
            Option<i64>,
            String,
            Option<i64>,
        )>(
            "SELECT id, sync_folder_id, relative_path, drive_file_id, drive_md5, drive_modified, local_md5, local_modified, sync_state, last_synced_at FROM file_states WHERE sync_folder_id = ? ORDER BY relative_path",
        )
        .bind(sync_folder_id)
        .fetch_all(pool)
        .await?;

        rows.into_iter()
            .map(row_to_file_state)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| sqlx::Error::Decode(Box::new(e)))
    }

    /// List file states by sync state kind.
    pub async fn list_by_state(
        pool: &SqlitePool,
        sync_folder_id: &str,
        kind: SyncStateKind,
    ) -> Result<Vec<FileState>, sqlx::Error> {
        let kind_str = kind_to_str(kind);
        let rows = sqlx::query_as::<_, (
            String,
            String,
            String,
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<String>,
            Option<i64>,
            String,
            Option<i64>,
        )>(
            "SELECT id, sync_folder_id, relative_path, drive_file_id, drive_md5, drive_modified, local_md5, local_modified, sync_state, last_synced_at FROM file_states WHERE sync_folder_id = ? AND sync_state LIKE ? ORDER BY relative_path",
        )
        .bind(sync_folder_id)
        .bind(format!("%\"kind\":\"{}\"%", kind_str))
        .fetch_all(pool)
        .await?;

        rows.into_iter()
            .map(row_to_file_state)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| sqlx::Error::Decode(Box::new(e)))
    }

    /// Delete file state by id.
    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM file_states WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Bulk upsert file states (in a transaction).
    pub async fn bulk_upsert(pool: &SqlitePool, states: &[FileState]) -> Result<(), sqlx::Error> {
        let mut tx = pool.begin().await?;
        for state in states {
            let drive_modified = state.drive_modified.map(|t| t.timestamp_millis());
            let local_modified = state.local_modified.map(|t| t.timestamp_millis());
            let last_synced_at = state.last_synced_at.map(|t| t.timestamp());
            let sync_state_json = sync_state_to_json(&state.sync_state);

            sqlx::query(
                r#"
                INSERT INTO file_states (id, sync_folder_id, relative_path, drive_file_id, drive_md5, drive_modified, local_md5, local_modified, sync_state, last_synced_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(sync_folder_id, relative_path) DO UPDATE SET
                    id = excluded.id,
                    drive_file_id = excluded.drive_file_id,
                    drive_md5 = excluded.drive_md5,
                    drive_modified = excluded.drive_modified,
                    local_md5 = excluded.local_md5,
                    local_modified = excluded.local_modified,
                    sync_state = excluded.sync_state,
                    last_synced_at = excluded.last_synced_at
                "#,
            )
            .bind(&state.id)
            .bind(&state.sync_folder_id)
            .bind(&state.relative_path)
            .bind(&state.drive_file_id)
            .bind(&state.drive_md5)
            .bind(drive_modified)
            .bind(&state.local_md5)
            .bind(local_modified)
            .bind(&sync_state_json)
            .bind(last_synced_at)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}

fn timestamp_millis_to_datetime(ms: i64) -> Option<DateTime<Utc>> {
    let secs = ms / 1000;
    let nsecs = ((ms % 1000) * 1_000_000) as u32;
    DateTime::from_timestamp(secs, nsecs)
}

fn kind_to_str(k: SyncStateKind) -> &'static str {
    match k {
        SyncStateKind::Synced => "synced",
        SyncStateKind::Pending => "pending",
        SyncStateKind::Conflict => "conflict",
        SyncStateKind::Error => "error",
        SyncStateKind::Uploading => "uploading",
        SyncStateKind::Downloading => "downloading",
    }
}

fn row_to_file_state(
    r: (
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<String>,
        Option<i64>,
        String,
        Option<i64>,
    ),
) -> Result<FileState, serde_json::Error> {
    let (
        id,
        sync_folder_id,
        relative_path,
        drive_file_id,
        drive_md5,
        drive_modified,
        local_md5,
        local_modified,
        sync_state_json,
        last_synced_at,
    ) = r;

    let drive_modified = drive_modified.and_then(timestamp_millis_to_datetime);
    let local_modified = local_modified.and_then(timestamp_millis_to_datetime);
    let last_synced_at = last_synced_at.and_then(|s| DateTime::from_timestamp_secs(s));
    let sync_state = sync_state_from_json(&sync_state_json)?;

    Ok(FileState {
        id,
        sync_folder_id,
        relative_path,
        drive_file_id,
        drive_md5,
        drive_modified,
        local_md5,
        local_modified,
        sync_state,
        last_synced_at,
    })
}
