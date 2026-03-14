//! Sync error log repository.

use chrono::{DateTime, Utc};

use sqlx::SqlitePool;

/// A row in the sync_errors table (error log entry).
#[derive(Clone, Debug)]
pub struct SyncErrorRecord {
    pub id: String,
    pub file_state_id: Option<String>,
    pub error_message: String,
    pub occurred_at: DateTime<Utc>,
    pub retry_count: i32,
}

/// Sync error log persistence.
pub struct SyncErrorRepository;

impl SyncErrorRepository {
    /// Insert an error record.
    pub async fn insert(
        pool: &SqlitePool,
        id: &str,
        file_state_id: Option<&str>,
        error_message: &str,
        occurred_at: DateTime<Utc>,
        retry_count: i32,
    ) -> Result<(), sqlx::Error> {
        let ts = occurred_at.timestamp();
        sqlx::query(
            r#"
            INSERT INTO sync_errors (id, file_state_id, error_message, occurred_at, retry_count)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(id)
        .bind(file_state_id)
        .bind(error_message)
        .bind(ts)
        .bind(retry_count)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Get recent errors (e.g. last N for a file or folder).
    pub async fn get_recent(
        pool: &SqlitePool,
        file_state_id: Option<&str>,
        limit: i64,
    ) -> Result<Vec<SyncErrorRecord>, sqlx::Error> {
        let rows = match file_state_id {
            Some(fid) => {
                sqlx::query_as::<_, (String, Option<String>, String, i64, i32)>(
                    "SELECT id, file_state_id, error_message, occurred_at, retry_count FROM sync_errors WHERE file_state_id = ? ORDER BY occurred_at DESC LIMIT ?",
                )
                .bind(fid)
                .bind(limit)
                .fetch_all(pool)
                .await?
            }
            None => {
                sqlx::query_as::<_, (String, Option<String>, String, i64, i32)>(
                    "SELECT id, file_state_id, error_message, occurred_at, retry_count FROM sync_errors ORDER BY occurred_at DESC LIMIT ?",
                )
                .bind(limit)
                .fetch_all(pool)
                .await?
            }
        };

        Ok(rows
            .into_iter()
            .map(
                |(id, file_state_id, error_message, occurred_at, retry_count)| SyncErrorRecord {
                    id,
                    file_state_id,
                    error_message,
                    occurred_at: DateTime::from_timestamp_secs(occurred_at)
                        .unwrap_or_else(Utc::now),
                    retry_count,
                },
            )
            .collect())
    }

    /// Clear errors for a file state.
    pub async fn clear_for_file(pool: &SqlitePool, file_state_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM sync_errors WHERE file_state_id = ?")
            .bind(file_state_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Increment retry count for an error by id.
    pub async fn increment_retry(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE sync_errors SET retry_count = retry_count + 1 WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }
}
