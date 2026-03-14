//! Sync executor: runs queued sync operations with concurrency limit and pause support.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::Utc;
use sqlx::SqlitePool;
use tracing::instrument;

use crate::api::{export_mime_type, is_google_workspace_file};
use crate::api::{CreateFileMetadata, DriveClient, SIMPLE_UPLOAD_MAX_BYTES};
use crate::auth::TokenProvider;
use crate::db::FileStateRepository;
use crate::model::{Config, DriveFile, FileState, SyncFolder, SyncState};
use crate::sync::change::SyncActionKind;
use crate::sync::fs::LocalFs;
use crate::sync::path::conflict_copy_path;
use crate::sync::queue::SyncQueue;
use crate::sync::workspace_stub::stub_content_for_mime;
use crate::sync::diff::parse_drive_modified;
use crate::model::SyncError;

/// Executes sync actions from a queue. Checks pause flag between operations.
#[allow(dead_code)]
pub struct SyncExecutor {
    drive_client: DriveClient,
    token_provider: std::sync::Arc<TokenProvider>,
    account_key: String,
    local_fs: std::sync::Arc<dyn LocalFs>,
    pool: SqlitePool,
    config: Config,
    max_concurrent_uploads: u32,
    max_concurrent_downloads: u32,
}

impl SyncExecutor {
    pub fn new(
        drive_client: DriveClient,
        token_provider: std::sync::Arc<TokenProvider>,
        account_key: String,
        local_fs: std::sync::Arc<dyn LocalFs>,
        pool: SqlitePool,
        config: Config,
    ) -> Self {
        Self {
            max_concurrent_uploads: config.sync.max_concurrent_uploads,
            max_concurrent_downloads: config.sync.max_concurrent_downloads,
            drive_client,
            token_provider,
            account_key,
            local_fs,
            pool,
            config,
        }
    }

    /// Runs the queue until empty or error. Checks `pause` between each action.
    #[instrument(skip(self, queue, pause), level = "info")]
    pub async fn run(
        &self,
        sync_folder: &SyncFolder,
        queue: &mut SyncQueue,
        pause: &AtomicBool,
    ) -> Result<u32, SyncError> {
        let sync_root = Path::new(&sync_folder.local_path);
        let mut executed = 0u32;
        while let Some(action) = queue.pop() {
            if pause.load(Ordering::Relaxed) {
                tracing::debug!("Sync paused");
                break;
            }
            self.execute_one(sync_root, sync_folder, &action).await?;
            executed += 1;
        }
        Ok(executed)
    }

    #[instrument(skip(self, action), level = "debug")]
    pub async fn execute_one(
        &self,
        sync_root: &Path,
        sync_folder: &SyncFolder,
        action: &crate::sync::change::SyncAction,
    ) -> Result<(), SyncError> {
        let token = self
            .token_provider
            .get_valid_access_token(&self.account_key)
            .await?;
        let sync_folder_id = &sync_folder.id;
        let drive_folder_id = &sync_folder.drive_folder_id;

        match &action.kind {
            SyncActionKind::NewUpload => {
                let content = self.local_fs.read_file(sync_root, &action.relative_path).await?;
                let mime = mime_guess::from_path(&action.relative_path).first_or_octet_stream().to_string();
                let meta = CreateFileMetadata {
                    name: Some(action.relative_path.clone()),
                    mime_type: Some(mime.clone()),
                    parents: Some(vec![drive_folder_id.clone()]),
                };
                let file = if content.len() as u64 <= SIMPLE_UPLOAD_MAX_BYTES {
                    self.drive_client
                        .files_create_simple(&token, &meta, &content, &mime)
                        .await?
                } else {
                    let mut cursor = std::io::Cursor::new(&content);
                    self.drive_client
                        .files_create_resumable(&token, &meta, content.len() as u64, &mime, &mut cursor, None)
                        .await?
                };
                let state = file_state_after_upload(sync_folder_id, &action.relative_path, &file, action.local_md5.as_deref(), action.local_modified);
                FileStateRepository::upsert(&self.pool, &state).await.map_err(|e| SyncError::DatabaseError(Box::new(e)))?;
            }
            SyncActionKind::UpdateUpload => {
                let state = action.state.as_ref().ok_or_else(|| SyncError::ApiError {
                    code: 0,
                    message: "UpdateUpload missing state".to_string(),
                })?;
                let content = self.local_fs.read_file(sync_root, &action.relative_path).await?;
                let mime = mime_guess::from_path(&action.relative_path).first_or_octet_stream().to_string();
                let file_id = state.drive_file_id.as_ref().ok_or_else(|| SyncError::ApiError {
                    code: 0,
                    message: "UpdateUpload missing drive_file_id".to_string(),
                })?;
                let file = if content.len() as u64 <= SIMPLE_UPLOAD_MAX_BYTES {
                    self.drive_client
                        .files_update_content_simple(&token, file_id, &content, &mime)
                        .await?
                } else {
                    let mut cursor = std::io::Cursor::new(&content);
                    self.drive_client
                        .files_update_content_resumable(&token, file_id, content.len() as u64, &mime, 0, &mut cursor, None)
                        .await?
                };
                let new_state = file_state_after_upload(sync_folder_id, &action.relative_path, &file, action.local_md5.as_deref(), action.local_modified);
                FileStateRepository::upsert(&self.pool, &new_state).await.map_err(|e| SyncError::DatabaseError(Box::new(e)))?;
            }
            SyncActionKind::NewDownload | SyncActionKind::UpdateDownload => {
                let drive_file = action.drive_file.as_ref().ok_or_else(|| SyncError::ApiError {
                    code: 0,
                    message: "Download action missing drive_file".to_string(),
                })?;
                let content = if is_google_workspace_file(&drive_file.mime_type) {
                    stub_content_for_mime(&drive_file.id, &drive_file.mime_type)
                        .unwrap_or_default()
                        .into_bytes()
                } else {
                    let mut buf = Vec::new();
                    if let Some(export_mime) = export_mime_type(&drive_file.mime_type) {
                        self.drive_client
                            .files_export(&token, &drive_file.id, export_mime, &mut buf)
                            .await?;
                    } else {
                        self.drive_client
                            .files_get_media(&token, &drive_file.id, &mut buf)
                            .await?;
                    }
                    buf
                };
                self.local_fs
                    .write_atomic(sync_root, &action.relative_path, &content)
                    .await?;
                let state = file_state_after_download(sync_folder_id, &action.relative_path, drive_file, action.state.as_ref());
                FileStateRepository::upsert(&self.pool, &state).await.map_err(|e| SyncError::DatabaseError(Box::new(e)))?;
            }
            SyncActionKind::Conflict => {
                let state = action.state.as_ref().ok_or_else(|| SyncError::ApiError {
                    code: 0,
                    message: "Conflict action missing state".to_string(),
                })?;
                let drive_file = action.drive_file.as_ref().ok_or_else(|| SyncError::ApiError {
                    code: 0,
                    message: "Conflict action missing drive_file".to_string(),
                })?;
                let local_content = self.local_fs.read_file(sync_root, &action.relative_path).await?;
                let full_path = sync_root.join(&action.relative_path);
                let conflict_path = conflict_copy_path(&full_path, Utc::now(), |p: &Path| p.exists());
                let conflict_rel = conflict_path.strip_prefix(sync_root).map(|p| p.to_string_lossy().to_string()).unwrap_or_else(|_| conflict_path.display().to_string());
                self.local_fs
                    .write_atomic(sync_root, &conflict_rel, &local_content)
                    .await?;
                let server_content = if is_google_workspace_file(&drive_file.mime_type) {
                    stub_content_for_mime(&drive_file.id, &drive_file.mime_type)
                        .unwrap_or_default()
                        .into_bytes()
                } else {
                    let mut buf = Vec::new();
                    if let Some(export_mime) = export_mime_type(&drive_file.mime_type) {
                        self.drive_client
                            .files_export(&token, &drive_file.id, export_mime, &mut buf)
                            .await?;
                    } else {
                        self.drive_client
                            .files_get_media(&token, &drive_file.id, &mut buf)
                            .await?;
                    }
                    buf
                };
                self.local_fs
                    .write_atomic(sync_root, &action.relative_path, &server_content)
                    .await?;
                let mut new_state = file_state_after_download(sync_folder_id, &action.relative_path, drive_file, Some(state));
                new_state.sync_state = SyncState::conflict();
                FileStateRepository::upsert(&self.pool, &new_state).await.map_err(|e| SyncError::DatabaseError(Box::new(e)))?;
                let _ = conflict_rel;
            }
            SyncActionKind::DeleteRemote => {
                let state = action.state.as_ref().ok_or_else(|| SyncError::ApiError {
                    code: 0,
                    message: "DeleteRemote missing state".to_string(),
                })?;
                let file_id = state.drive_file_id.as_ref().ok_or_else(|| SyncError::ApiError {
                    code: 0,
                    message: "DeleteRemote missing drive_file_id".to_string(),
                })?;
                self.drive_client.files_delete(&token, file_id).await?;
                FileStateRepository::delete(&self.pool, &state.id).await.map_err(|e| SyncError::DatabaseError(Box::new(e)))?;
            }
            SyncActionKind::DeleteLocal => {
                let state = action.state.as_ref().ok_or_else(|| SyncError::ApiError {
                    code: 0,
                    message: "DeleteLocal missing state".to_string(),
                })?;
                self.local_fs.remove_file(sync_root, &action.relative_path).await?;
                FileStateRepository::delete(&self.pool, &state.id).await.map_err(|e| SyncError::DatabaseError(Box::new(e)))?;
            }
        }
        Ok(())
    }
}

fn file_state_after_upload(
    sync_folder_id: &str,
    relative_path: &str,
    file: &DriveFile,
    local_md5: Option<&str>,
    local_modified: Option<chrono::DateTime<Utc>>,
) -> FileState {
    let now = Utc::now();
    FileState {
        id: uuid::Uuid::new_v4().to_string(),
        sync_folder_id: sync_folder_id.to_string(),
        relative_path: relative_path.to_string(),
        drive_file_id: Some(file.id.clone()),
        drive_md5: file.md5_checksum.clone(),
        drive_modified: parse_drive_modified(file.modified_time.as_deref()),
        local_md5: local_md5.map(String::from),
        local_modified,
        sync_state: SyncState::synced(),
        last_synced_at: Some(now),
    }
}

fn file_state_after_download(
    sync_folder_id: &str,
    relative_path: &str,
    file: &DriveFile,
    existing: Option<&FileState>,
) -> FileState {
    let now = Utc::now();
    let id = existing.map(|s| s.id.clone()).unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    FileState {
        id,
        sync_folder_id: sync_folder_id.to_string(),
        relative_path: relative_path.to_string(),
        drive_file_id: Some(file.id.clone()),
        drive_md5: file.md5_checksum.clone(),
        drive_modified: parse_drive_modified(file.modified_time.as_deref()),
        local_md5: file.md5_checksum.clone(),
        local_modified: parse_drive_modified(file.modified_time.as_deref()),
        sync_state: SyncState::synced(),
        last_synced_at: Some(now),
    }
}
