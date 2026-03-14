//! Sync folder repository.

use chrono::DateTime;

use sqlx::SqlitePool;

use crate::model::SyncFolder;

/// Sync folder persistence.
pub struct SyncFolderRepository;

impl SyncFolderRepository {
    /// Insert a new sync folder.
    pub async fn insert(pool: &SqlitePool, folder: &SyncFolder) -> Result<(), sqlx::Error> {
        let last_sync_at = folder.last_sync_at.map(|t| t.timestamp());
        let paused = if folder.paused { 1 } else { 0 };
        sqlx::query(
            r#"
            INSERT INTO sync_folders (id, account_id, local_path, drive_folder_id, start_page_token, last_sync_at, paused)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&folder.id)
        .bind(&folder.account_id)
        .bind(&folder.local_path)
        .bind(&folder.drive_folder_id)
        .bind(&folder.start_page_token)
        .bind(last_sync_at)
        .bind(paused)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Get sync folder by id.
    pub async fn get_by_id(pool: &SqlitePool, id: &str) -> Result<Option<SyncFolder>, sqlx::Error> {
        let row = sqlx::query_as::<_, (
            String,
            String,
            String,
            String,
            Option<String>,
            Option<i64>,
            i64,
        )>(
            "SELECT id, account_id, local_path, drive_folder_id, start_page_token, last_sync_at, paused FROM sync_folders WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(
            |(
                id,
                account_id,
                local_path,
                drive_folder_id,
                start_page_token,
                last_sync_at,
                paused,
            )| {
                SyncFolder {
                    id,
                    account_id,
                    local_path,
                    drive_folder_id,
                    start_page_token,
                    last_sync_at: last_sync_at.and_then(|t| DateTime::from_timestamp_secs(t)),
                    paused: paused != 0,
                }
            },
        ))
    }

    /// List sync folders for an account.
    pub async fn list_by_account(
        pool: &SqlitePool,
        account_id: &str,
    ) -> Result<Vec<SyncFolder>, sqlx::Error> {
        let rows = sqlx::query_as::<_, (
            String,
            String,
            String,
            String,
            Option<String>,
            Option<i64>,
            i64,
        )>(
            "SELECT id, account_id, local_path, drive_folder_id, start_page_token, last_sync_at, paused FROM sync_folders WHERE account_id = ? ORDER BY id",
        )
        .bind(account_id)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(
                    id,
                    account_id,
                    local_path,
                    drive_folder_id,
                    start_page_token,
                    last_sync_at,
                    paused,
                )| {
                    SyncFolder {
                        id,
                        account_id,
                        local_path,
                        drive_folder_id,
                        start_page_token,
                        last_sync_at: last_sync_at.and_then(|t| DateTime::from_timestamp_secs(t)),
                        paused: paused != 0,
                    }
                },
            )
            .collect())
    }

    /// Delete sync folder by id.
    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM sync_folders WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Set paused flag.
    pub async fn set_paused(pool: &SqlitePool, id: &str, paused: bool) -> Result<(), sqlx::Error> {
        let p = if paused { 1 } else { 0 };
        sqlx::query("UPDATE sync_folders SET paused = ? WHERE id = ?")
            .bind(p)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Update start page token.
    pub async fn update_page_token(
        pool: &SqlitePool,
        id: &str,
        token: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE sync_folders SET start_page_token = ? WHERE id = ?")
            .bind(token)
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
