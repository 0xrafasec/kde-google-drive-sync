//! Integration tests for sync engine: DiffEngine, merge, path validation.

use chrono::Utc;
use gds_core::db::{create_pool, run_migrations, AccountRepository, SyncFolderRepository};
use gds_core::model::{Account, ChangeSet, DriveChange, DriveFile, FileState, SyncFolder, SyncState};
use gds_core::sync::{
    is_conflict, DiffEngine, DirEntry, LocalFileMeta, LocalFs, SyncAction, SyncActionKind,
    parse_drive_modified, safe_local_path,
};
use std::collections::HashMap;
use std::path::Path;

struct MockLocalFs {
    files: HashMap<String, (String, chrono::DateTime<Utc>, bool)>,
}

#[async_trait::async_trait]
impl LocalFs for MockLocalFs {
    async fn list_dir(&self, _sync_root: &Path, relative_path: &str) -> Result<Vec<DirEntry>, gds_core::model::SyncError> {
        let mut entries = Vec::new();
        for (path, (_, _, is_dir)) in &self.files {
            let (parent, name) = if let Some((p, n)) = path.rsplit_once('/') {
                (p, n.to_string())
            } else {
                if !relative_path.is_empty() {
                    continue;
                }
                (relative_path, path.clone())
            };
            if parent == relative_path {
                entries.push(DirEntry {
                    name,
                    is_dir: *is_dir,
                });
            }
        }
        Ok(entries)
    }

    async fn file_metadata(
        &self,
        _sync_root: &Path,
        relative_path: &str,
    ) -> Result<Option<LocalFileMeta>, gds_core::model::SyncError> {
        Ok(self.files.get(relative_path).map(|(md5, modified, is_dir)| {
            LocalFileMeta {
                md5: md5.clone(),
                modified: *modified,
                is_dir: *is_dir,
                size: 0,
            }
        }))
    }

    async fn read_file(&self, _sync_root: &Path, _relative_path: &str) -> Result<Vec<u8>, gds_core::model::SyncError> {
        Ok(vec![])
    }

    async fn write_atomic(
        &self,
        _sync_root: &Path,
        _relative_path: &str,
        _content: &[u8],
    ) -> Result<(), gds_core::model::SyncError> {
        Ok(())
    }

    async fn create_dir_all(&self, _sync_root: &Path, _relative_path: &str) -> Result<(), gds_core::model::SyncError> {
        Ok(())
    }

    async fn remove_file(&self, _sync_root: &Path, _relative_path: &str) -> Result<(), gds_core::model::SyncError> {
        Ok(())
    }

    async fn remove_dir(&self, _sync_root: &Path, _relative_path: &str) -> Result<(), gds_core::model::SyncError> {
        Ok(())
    }

    async fn exists(&self, _sync_root: &Path, relative_path: &str) -> Result<bool, gds_core::model::SyncError> {
        Ok(self.files.contains_key(relative_path))
    }

    async fn is_external_symlink(&self, _sync_root: &Path, _full_path: &Path) -> Result<bool, gds_core::model::SyncError> {
        Ok(false)
    }
}

#[tokio::test]
async fn diff_engine_compute_local_changes_new_upload() {
    let dir = tempfile::tempdir().unwrap();
    let sync_root = dir.path();

    let pool = create_pool("sqlite::memory:").await.unwrap();
    run_migrations(&pool).await.unwrap();

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
        local_path: sync_root.to_string_lossy().to_string(),
        drive_folder_id: "drive-root".to_string(),
        start_page_token: None,
        last_sync_at: None,
        paused: false,
    };
    SyncFolderRepository::insert(&pool, &folder).await.unwrap();

    let fs = MockLocalFs {
        files: HashMap::from([
            ("a.txt".to_string(), ("md5a".to_string(), Utc::now(), false)),
        ]),
    };

    let actions = DiffEngine::compute_local_changes(sync_root, &folder, &pool, &fs).await.unwrap();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].kind, SyncActionKind::NewUpload);
    assert_eq!(actions[0].relative_path, "a.txt");
}

#[tokio::test]
async fn diff_engine_compute_remote_changes_new_download() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    run_migrations(&pool).await.unwrap();

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

    let folder = SyncFolder {
        id: "sf-1".to_string(),
        account_id: "acc-1".to_string(),
        local_path: "/tmp/sync".to_string(),
        drive_folder_id: "drive-root".to_string(),
        start_page_token: Some("token".to_string()),
        last_sync_at: None,
        paused: false,
    };
    SyncFolderRepository::insert(&pool, &folder).await.unwrap();

    let change_set = ChangeSet {
        next_page_token: None,
        new_start_page_token: None,
        changes: vec![DriveChange {
            change_type: "file".to_string(),
            file_id: "f1".to_string(),
            file: Some(DriveFile {
                id: "f1".to_string(),
                name: "new.txt".to_string(),
                mime_type: "text/plain".to_string(),
                md5_checksum: Some("m1".to_string()),
                size: Some("10".to_string()),
                modified_time: Some("2024-01-01T12:00:00.000Z".to_string()),
                parents: Some(vec!["drive-root".to_string()]),
                trashed: Some(false),
            }),
            removed: Some(false),
        }],
    };

    let actions = DiffEngine::compute_remote_changes(&folder, &change_set, &pool).await.unwrap();
    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].kind, SyncActionKind::NewDownload);
    assert_eq!(actions[0].relative_path, "new.txt");
}

#[tokio::test]
async fn diff_engine_merge_actions_conflict() {

    let t = Utc::now();
    let state = FileState {
        id: "id".to_string(),
        sync_folder_id: "sf".to_string(),
        relative_path: "f.txt".to_string(),
        drive_file_id: Some("did".to_string()),
        drive_md5: Some("d1".to_string()),
        drive_modified: Some(t),
        local_md5: Some("m1".to_string()),
        local_modified: Some(t),
        sync_state: SyncState::synced(),
        last_synced_at: Some(t),
    };
    let drive_file = DriveFile {
        id: "did".to_string(),
        name: "f.txt".to_string(),
        mime_type: "text/plain".to_string(),
        md5_checksum: Some("d2".to_string()),
        size: None,
        modified_time: Some("2024-02-01T00:00:00.000Z".to_string()),
        parents: None,
        trashed: None,
    };

    let local = vec![SyncAction::update_upload(
        "f.txt".to_string(),
        state.clone(),
        "m2".to_string(),
        t,
    )];
    let remote = vec![SyncAction::update_download(
        "f.txt".to_string(),
        state,
        drive_file,
    )];

    let merged = DiffEngine::merge_actions(local, remote);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].kind, SyncActionKind::Conflict);
}

#[test]
fn path_validation_traversal_sanitized() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let p = safe_local_path(root, "../../../etc/passwd").unwrap();
    assert!(p.starts_with(root));
    assert_eq!(p, root.join("etc/passwd"));
}

#[test]
fn is_conflict_detection() {
    let t = Utc::now();
    let state = FileState {
        id: "i".to_string(),
        sync_folder_id: "sf".to_string(),
        relative_path: "f".to_string(),
        drive_file_id: Some("d".to_string()),
        drive_md5: Some("d1".to_string()),
        drive_modified: Some(t),
        local_md5: Some("m1".to_string()),
        local_modified: Some(t),
        sync_state: SyncState::synced(),
        last_synced_at: Some(t),
    };
    assert!(is_conflict(&state, "m2", t, Some("d2"), Some(t)));
    assert!(!is_conflict(&state, "m1", t, Some("d1"), Some(t)));
}

#[test]
fn parse_drive_modified_integration() {
    let t = parse_drive_modified(Some("2024-06-15T14:30:00.000Z"));
    assert!(t.is_some());
}
