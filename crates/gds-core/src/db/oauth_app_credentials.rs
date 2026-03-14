//! OAuth app credentials (client_id, client_secret) stored in the DB.
//! Single row so the daemon can resolve credentials without keyring.

use sqlx::SqlitePool;

/// Get stored OAuth app credentials, if any.
pub async fn get(pool: &SqlitePool) -> Result<Option<(String, String)>, sqlx::Error> {
    let row = sqlx::query_as::<_, (String, String)>(
        "SELECT client_id, client_secret FROM oauth_app_credentials WHERE id = 1",
    )
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

/// Store or replace OAuth app credentials (single row).
pub async fn upsert(
    pool: &SqlitePool,
    client_id: &str,
    client_secret: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO oauth_app_credentials (id, client_id, client_secret)
        VALUES (1, ?, ?)
        ON CONFLICT(id) DO UPDATE SET client_id = excluded.client_id, client_secret = excluded.client_secret
        "#,
    )
    .bind(client_id)
    .bind(client_secret)
    .execute(pool)
    .await?;
    Ok(())
}
