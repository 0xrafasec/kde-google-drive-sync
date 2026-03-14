//! Database layer: SQLite pool, migrations, repositories.

mod account;
mod file_state;
mod oauth_app_credentials;
mod sync_error;
mod sync_folder;

use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool};

pub use account::AccountRepository;
pub use file_state::FileStateRepository;
pub use oauth_app_credentials::{get as get_oauth_app_credentials, upsert as upsert_oauth_app_credentials};
pub use sync_error::{SyncErrorRecord, SyncErrorRepository};
pub use sync_folder::SyncFolderRepository;

/// Creates a SQLite pool with WAL mode and busy timeout.
/// Use `sqlite::memory:` or `sqlite://path/to/db` for the URL.
pub async fn create_pool(url: &str) -> Result<SqlitePool, sqlx::Error> {
    let opts = SqliteConnectOptions::from_str(url)?
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_secs(30))
        .create_if_missing(true);
    SqlitePool::connect_with(opts).await
}

/// Creates a pool for a database file at the given path.
pub async fn create_pool_from_path(path: &Path) -> Result<SqlitePool, sqlx::Error> {
    let url = format!("sqlite:{}", path.display());
    create_pool(&url).await
}

/// Runs all migrations. Call after creating the pool.
pub async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::migrate::MigrateError> {
    // Path relative to crate root (where Cargo.toml is).
    sqlx::migrate!("./migrations").run(pool).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{
        AccountRepository, FileStateRepository, SyncErrorRepository, SyncFolderRepository,
    };
    use crate::model::{Account, FileState, SyncFolder, SyncState};
    use chrono::Utc;

    async fn test_pool() -> SqlitePool {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        run_migrations(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn test_account_insert_get_list_delete() {
        let pool = test_pool().await;
        let account = Account {
            id: "acc-1".to_string(),
            email: "u@example.com".to_string(),
            display_name: Some("User".to_string()),
            keyring_key: "gds:acc-1".to_string(),
            created_at: Utc::now(),
        };
        AccountRepository::insert(&pool, &account).await.unwrap();
        let got = AccountRepository::get_by_id(&pool, "acc-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got.id, account.id);
        assert_eq!(got.email, account.email);
        let list = AccountRepository::list_all(&pool).await.unwrap();
        assert_eq!(list.len(), 1);
        AccountRepository::delete(&pool, "acc-1").await.unwrap();
        assert!(AccountRepository::get_by_id(&pool, "acc-1")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn test_account_delete_cascade() {
        let pool = test_pool().await;
        let account = Account {
            id: "acc-1".to_string(),
            email: "u@example.com".to_string(),
            display_name: None,
            keyring_key: "k".to_string(),
            created_at: Utc::now(),
        };
        AccountRepository::insert(&pool, &account).await.unwrap();
        let folder = SyncFolder {
            id: "sf-1".to_string(),
            account_id: "acc-1".to_string(),
            local_path: "/tmp/d".to_string(),
            drive_folder_id: "df".to_string(),
            start_page_token: None,
            last_sync_at: None,
            paused: false,
        };
        SyncFolderRepository::insert(&pool, &folder).await.unwrap();
        let state = FileState::new_pending("fs-1".into(), "sf-1".into(), "a/b".into());
        FileStateRepository::upsert(&pool, &state).await.unwrap();
        SyncErrorRepository::insert(&pool, "err-1", Some("fs-1"), "msg", Utc::now(), 0)
            .await
            .unwrap();
        AccountRepository::delete_cascade(&pool, "acc-1")
            .await
            .unwrap();
        assert!(AccountRepository::get_by_id(&pool, "acc-1")
            .await
            .unwrap()
            .is_none());
        assert!(SyncFolderRepository::get_by_id(&pool, "sf-1")
            .await
            .unwrap()
            .is_none());
        assert!(FileStateRepository::get_by_path(&pool, "sf-1", "a/b")
            .await
            .unwrap()
            .is_none());
        let errs = SyncErrorRepository::get_recent(&pool, None, 10)
            .await
            .unwrap();
        assert!(errs.is_empty());
    }

    #[tokio::test]
    async fn test_sync_folder_insert_get_list_delete_set_paused_update_token() {
        let pool = test_pool().await;
        let account = Account {
            id: "acc-1".to_string(),
            email: "u@example.com".to_string(),
            display_name: None,
            keyring_key: "k".to_string(),
            created_at: Utc::now(),
        };
        AccountRepository::insert(&pool, &account).await.unwrap();
        let folder = SyncFolder {
            id: "sf-1".to_string(),
            account_id: "acc-1".to_string(),
            local_path: "/tmp/d".to_string(),
            drive_folder_id: "df".to_string(),
            start_page_token: None,
            last_sync_at: None,
            paused: false,
        };
        SyncFolderRepository::insert(&pool, &folder).await.unwrap();
        let got = SyncFolderRepository::get_by_id(&pool, "sf-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got.local_path, "/tmp/d");
        let list = SyncFolderRepository::list_by_account(&pool, "acc-1")
            .await
            .unwrap();
        assert_eq!(list.len(), 1);
        SyncFolderRepository::set_paused(&pool, "sf-1", true)
            .await
            .unwrap();
        let got2 = SyncFolderRepository::get_by_id(&pool, "sf-1")
            .await
            .unwrap()
            .unwrap();
        assert!(got2.paused);
        SyncFolderRepository::update_page_token(&pool, "sf-1", Some("token123"))
            .await
            .unwrap();
        let got3 = SyncFolderRepository::get_by_id(&pool, "sf-1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got3.start_page_token.as_deref(), Some("token123"));
        SyncFolderRepository::delete(&pool, "sf-1").await.unwrap();
        assert!(SyncFolderRepository::get_by_id(&pool, "sf-1")
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn test_file_state_upsert_get_by_path_list_by_folder_list_by_state_delete_bulk_upsert() {
        let pool = test_pool().await;
        let account = Account {
            id: "acc-1".to_string(),
            email: "u@example.com".to_string(),
            display_name: None,
            keyring_key: "k".to_string(),
            created_at: Utc::now(),
        };
        AccountRepository::insert(&pool, &account).await.unwrap();
        let folder = SyncFolder {
            id: "sf-1".to_string(),
            account_id: "acc-1".to_string(),
            local_path: "/tmp/d".to_string(),
            drive_folder_id: "df".to_string(),
            start_page_token: None,
            last_sync_at: None,
            paused: false,
        };
        SyncFolderRepository::insert(&pool, &folder).await.unwrap();

        let state = FileState::new_pending("id1".into(), "sf-1".into(), "p/file.txt".into());
        FileStateRepository::upsert(&pool, &state).await.unwrap();
        let got = FileStateRepository::get_by_path(&pool, "sf-1", "p/file.txt")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got.id, "id1");
        assert_eq!(got.relative_path, "p/file.txt");

        let list = FileStateRepository::list_by_folder(&pool, "sf-1")
            .await
            .unwrap();
        assert_eq!(list.len(), 1);

        let mut state2 = state.clone();
        state2.sync_state = SyncState::synced();
        state2.id = "id1".to_string();
        FileStateRepository::upsert(&pool, &state2).await.unwrap();
        let list_pending =
            FileStateRepository::list_by_state(&pool, "sf-1", crate::model::SyncStateKind::Pending)
                .await
                .unwrap();
        let list_synced =
            FileStateRepository::list_by_state(&pool, "sf-1", crate::model::SyncStateKind::Synced)
                .await
                .unwrap();
        assert!(list_pending.is_empty());
        assert_eq!(list_synced.len(), 1);

        FileStateRepository::delete(&pool, "id1").await.unwrap();
        assert!(
            FileStateRepository::get_by_path(&pool, "sf-1", "p/file.txt")
                .await
                .unwrap()
                .is_none()
        );

        let bulk = vec![
            FileState::new_pending("b1".into(), "sf-1".into(), "x/1".into()),
            FileState::new_pending("b2".into(), "sf-1".into(), "x/2".into()),
        ];
        FileStateRepository::bulk_upsert(&pool, &bulk)
            .await
            .unwrap();
        let list_after = FileStateRepository::list_by_folder(&pool, "sf-1")
            .await
            .unwrap();
        assert_eq!(list_after.len(), 2);
    }

    #[tokio::test]
    async fn test_sync_error_insert_get_recent_clear_for_file_increment_retry() {
        let pool = test_pool().await;
        // Create account and folder and file_state so sync_errors can reference fs-1
        AccountRepository::insert(
            &pool,
            &Account {
                id: "acc-1".to_string(),
                email: "u@example.com".to_string(),
                display_name: None,
                keyring_key: "k".to_string(),
                created_at: Utc::now(),
            },
        )
        .await
        .unwrap();
        SyncFolderRepository::insert(
            &pool,
            &SyncFolder {
                id: "sf-1".to_string(),
                account_id: "acc-1".to_string(),
                local_path: "/tmp/d".to_string(),
                drive_folder_id: "df".to_string(),
                start_page_token: None,
                last_sync_at: None,
                paused: false,
            },
        )
        .await
        .unwrap();
        FileStateRepository::upsert(
            &pool,
            &FileState::new_pending("fs-1".into(), "sf-1".into(), "a".into()),
        )
        .await
        .unwrap();

        SyncErrorRepository::insert(&pool, "e1", None, "error one", Utc::now(), 0)
            .await
            .unwrap();
        SyncErrorRepository::insert(&pool, "e2", Some("fs-1"), "error two", Utc::now(), 1)
            .await
            .unwrap();
        let recent = SyncErrorRepository::get_recent(&pool, None, 10)
            .await
            .unwrap();
        assert_eq!(recent.len(), 2);
        let for_file = SyncErrorRepository::get_recent(&pool, Some("fs-1"), 10)
            .await
            .unwrap();
        assert_eq!(for_file.len(), 1);
        assert_eq!(for_file[0].retry_count, 1);

        SyncErrorRepository::increment_retry(&pool, "e2")
            .await
            .unwrap();
        let again = SyncErrorRepository::get_recent(&pool, Some("fs-1"), 10)
            .await
            .unwrap();
        assert_eq!(again[0].retry_count, 2);

        SyncErrorRepository::clear_for_file(&pool, "fs-1")
            .await
            .unwrap();
        let after_clear = SyncErrorRepository::get_recent(&pool, Some("fs-1"), 10)
            .await
            .unwrap();
        assert!(after_clear.is_empty());
    }
}
