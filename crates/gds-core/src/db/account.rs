//! Account repository.

use chrono::{DateTime, Utc};

use sqlx::SqlitePool;

use crate::model::Account;

/// Account persistence.
pub struct AccountRepository;

impl AccountRepository {
    /// Insert a new account.
    pub async fn insert(pool: &SqlitePool, account: &Account) -> Result<(), sqlx::Error> {
        let created_at = account.created_at.timestamp();
        sqlx::query(
            r#"
            INSERT INTO accounts (id, email, display_name, keyring_key, created_at)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&account.id)
        .bind(&account.email)
        .bind(&account.display_name)
        .bind(&account.keyring_key)
        .bind(created_at)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Get account by id.
    pub async fn get_by_id(pool: &SqlitePool, id: &str) -> Result<Option<Account>, sqlx::Error> {
        let row = sqlx::query_as::<_, (String, String, Option<String>, String, i64)>(
            "SELECT id, email, display_name, keyring_key, created_at FROM accounts WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(pool)
        .await?;

        Ok(row.map(
            |(id, email, display_name, keyring_key, created_at)| Account {
                id,
                email,
                display_name,
                keyring_key,
                created_at: DateTime::from_timestamp_secs(created_at).unwrap_or_else(Utc::now),
            },
        ))
    }

    /// List all accounts.
    pub async fn list_all(pool: &SqlitePool) -> Result<Vec<Account>, sqlx::Error> {
        let rows = sqlx::query_as::<_, (String, String, Option<String>, String, i64)>(
            "SELECT id, email, display_name, keyring_key, created_at FROM accounts ORDER BY created_at",
        )
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(id, email, display_name, keyring_key, created_at)| Account {
                    id,
                    email,
                    display_name,
                    keyring_key,
                    created_at: DateTime::from_timestamp_secs(created_at).unwrap_or_else(Utc::now),
                },
            )
            .collect())
    }

    /// Delete account by id.
    pub async fn delete(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM accounts WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Delete account and all related data in one transaction (sync_folders, file_states, sync_errors).
    pub async fn delete_cascade(pool: &SqlitePool, id: &str) -> Result<(), sqlx::Error> {
        let mut tx = pool.begin().await?;
        // Get sync folder ids for this account
        let folder_ids: Vec<String> =
            sqlx::query_scalar("SELECT id FROM sync_folders WHERE account_id = ?")
                .bind(id)
                .fetch_all(&mut *tx)
                .await?;
        for folder_id in &folder_ids {
            sqlx::query("DELETE FROM sync_errors WHERE file_state_id IN (SELECT id FROM file_states WHERE sync_folder_id = ?)")
                .bind(folder_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("DELETE FROM file_states WHERE sync_folder_id = ?")
                .bind(folder_id)
                .execute(&mut *tx)
                .await?;
        }
        sqlx::query("DELETE FROM sync_folders WHERE account_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM accounts WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}
