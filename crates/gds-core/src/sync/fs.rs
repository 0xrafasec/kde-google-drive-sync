//! Local filesystem abstraction for sync (trait + tokio implementation).
//!
//! Enables testing without real I/O and enforces symlink policy.

use std::path::Path;

use chrono::{DateTime, Utc};

use crate::model::SyncError;

/// Metadata for a local file (for diff).
#[derive(Clone, Debug)]
pub struct LocalFileMeta {
    pub md5: String,
    pub modified: DateTime<Utc>,
    pub is_dir: bool,
    pub size: u64,
}

/// Entry when listing a directory (name relative to parent, is_dir).
#[derive(Clone, Debug)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
}

/// Local filesystem operations used by the sync engine.
/// Implementations may use real I/O (TokioLocalFs) or mocks for tests.
#[async_trait::async_trait]
pub trait LocalFs: Send + Sync {
    /// Lists direct children of the given path (relative to sync root).
    /// Symlinks that point outside sync root should be skipped (SECURITY).
    async fn list_dir(&self, sync_root: &Path, relative_path: &str) -> Result<Vec<DirEntry>, SyncError>;

    /// Returns file metadata (md5, mtime). Returns None if not found or not a file.
    async fn file_metadata(
        &self,
        sync_root: &Path,
        relative_path: &str,
    ) -> Result<Option<LocalFileMeta>, SyncError>;

    /// Reads full file content. Use only for small files or streaming elsewhere.
    async fn read_file(
        &self,
        sync_root: &Path,
        relative_path: &str,
    ) -> Result<Vec<u8>, SyncError>;

    /// Writes content atomically (temp file + rename). Used for downloads.
    async fn write_atomic(
        &self,
        sync_root: &Path,
        relative_path: &str,
        content: &[u8],
    ) -> Result<(), SyncError>;

    /// Creates directory and parents. Idempotent.
    async fn create_dir_all(&self, sync_root: &Path, relative_path: &str) -> Result<(), SyncError>;

    /// Removes a file.
    async fn remove_file(&self, sync_root: &Path, relative_path: &str) -> Result<(), SyncError>;

    /// Removes an empty directory.
    async fn remove_dir(&self, sync_root: &Path, relative_path: &str) -> Result<(), SyncError>;

    /// Returns true if path exists (file or dir).
    async fn exists(&self, sync_root: &Path, relative_path: &str) -> Result<bool, SyncError>;

    /// Returns true if path is a symlink that points outside sync_root (skip during scan).
    async fn is_external_symlink(&self, sync_root: &Path, full_path: &Path) -> Result<bool, SyncError>;
}

/// Real implementation using tokio::fs. Respects symlink policy.
pub struct TokioLocalFs;

#[async_trait::async_trait]
impl LocalFs for TokioLocalFs {
    async fn list_dir(&self, sync_root: &Path, relative_path: &str) -> Result<Vec<DirEntry>, SyncError> {
        let path = if relative_path.is_empty() {
            sync_root.to_path_buf()
        } else {
            sync_root.join(relative_path)
        };
        let mut entries = Vec::new();
        let mut read_dir = tokio::fs::read_dir(&path).await.map_err(|e| SyncError::IoError {
            path: path.display().to_string(),
            source: e,
        })?;
        let sync_root_canon = sync_root
            .canonicalize()
            .map_err(|e| SyncError::IoError {
                path: sync_root.display().to_string(),
                source: e,
            })?;
        while let Some(entry) = read_dir.next_entry().await.map_err(|e| SyncError::IoError {
            path: path.display().to_string(),
            source: e,
        })? {
            let e = entry;
            let name = e.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with('.') && name_str == ".gds_tmp" {
                continue;
            }
            let full = e.path();
            let meta = tokio::fs::symlink_metadata(&full).await.map_err(|e| SyncError::IoError {
                path: full.display().to_string(),
                source: e,
            })?;
            if meta.file_type().is_symlink() {
                let target = tokio::fs::read_link(&full).await.map_err(|e| SyncError::IoError {
                    path: full.display().to_string(),
                    source: e,
                })?;
                if !target.starts_with(&sync_root_canon) {
                    tracing::debug!("Skipping external symlink: {}", full.display());
                    continue;
                }
            }
            entries.push(DirEntry {
                name: name_str.to_string(),
                is_dir: meta.file_type().is_dir(),
            });
        }
        Ok(entries)
    }

    async fn file_metadata(
        &self,
        sync_root: &Path,
        relative_path: &str,
    ) -> Result<Option<LocalFileMeta>, SyncError> {
        let path = sync_root.join(relative_path);
        let meta = match tokio::fs::symlink_metadata(&path).await {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(SyncError::IoError {
                    path: path.display().to_string(),
                    source: e,
                })
            }
        };
        if meta.file_type().is_dir() {
            let modified = meta
                .modified()
                .map_err(|e| SyncError::IoError {
                    path: path.display().to_string(),
                    source: e,
                })?
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| SyncError::IoError {
                    path: path.display().to_string(),
                    source: std::io::Error::new(std::io::ErrorKind::Other, e),
                })?;
            let modified_dt = DateTime::from_timestamp(modified.as_secs() as i64, modified.subsec_nanos())
                .unwrap_or_else(Utc::now);
            return Ok(Some(LocalFileMeta {
                md5: String::new(),
                modified: modified_dt,
                is_dir: true,
                size: 0,
            }));
        }
        if meta.file_type().is_symlink() {
            let sync_root_canon = sync_root.canonicalize().map_err(|e| SyncError::IoError {
                path: sync_root.display().to_string(),
                source: e,
            })?;
            let target = tokio::fs::read_link(&path).await.map_err(|e| SyncError::IoError {
                path: path.display().to_string(),
                source: e,
            })?;
            if !target.starts_with(&sync_root_canon) {
                return Ok(None);
            }
        }
        let modified = meta
            .modified()
            .map_err(|e| SyncError::IoError {
                path: path.display().to_string(),
                source: e,
            })?
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| SyncError::IoError {
                path: path.display().to_string(),
                source: std::io::Error::new(std::io::ErrorKind::Other, e),
            })?;
        let modified_dt = DateTime::from_timestamp(modified.as_secs() as i64, modified.subsec_nanos()).unwrap_or_else(Utc::now);
        let content = tokio::fs::read(&path).await.map_err(|e| SyncError::IoError {
            path: path.display().to_string(),
            source: e,
        })?;
        let md5 = format!("{:x}", md5::compute(&content));
        let size = meta.len();
        Ok(Some(LocalFileMeta {
            md5,
            modified: modified_dt,
            is_dir: false,
            size,
        }))
    }

    async fn read_file(
        &self,
        sync_root: &Path,
        relative_path: &str,
    ) -> Result<Vec<u8>, SyncError> {
        let path = sync_root.join(relative_path);
        tokio::fs::read(&path).await.map_err(|e| SyncError::IoError {
            path: path.display().to_string(),
            source: e,
        })
    }

    async fn write_atomic(
        &self,
        sync_root: &Path,
        relative_path: &str,
        content: &[u8],
    ) -> Result<(), SyncError> {
        let path = sync_root.join(relative_path);
        if let Some(p) = path.parent() {
            tokio::fs::create_dir_all(p).await.map_err(|e| SyncError::IoError {
                path: p.display().to_string(),
                source: e,
            })?;
        }
        let tmp = path.with_extension("gds_tmp");
        tokio::fs::write(&tmp, content).await.map_err(|e| SyncError::IoError {
            path: tmp.display().to_string(),
            source: e,
        })?;
        tokio::fs::rename(&tmp, &path).await.map_err(|e| SyncError::IoError {
            path: path.display().to_string(),
            source: e,
        })?;
        Ok(())
    }

    async fn create_dir_all(&self, sync_root: &Path, relative_path: &str) -> Result<(), SyncError> {
        let path = sync_root.join(relative_path);
        tokio::fs::create_dir_all(&path).await.map_err(|e| SyncError::IoError {
            path: path.display().to_string(),
            source: e,
        })?;
        Ok(())
    }

    async fn remove_file(&self, sync_root: &Path, relative_path: &str) -> Result<(), SyncError> {
        let path = sync_root.join(relative_path);
        tokio::fs::remove_file(&path).await.map_err(|e| SyncError::IoError {
            path: path.display().to_string(),
            source: e,
        })?;
        Ok(())
    }

    async fn remove_dir(&self, sync_root: &Path, relative_path: &str) -> Result<(), SyncError> {
        let path = sync_root.join(relative_path);
        tokio::fs::remove_dir(&path).await.map_err(|e| SyncError::IoError {
            path: path.display().to_string(),
            source: e,
        })?;
        Ok(())
    }

    async fn exists(&self, sync_root: &Path, relative_path: &str) -> Result<bool, SyncError> {
        let path = sync_root.join(relative_path);
        Ok(path.try_exists().map_err(|e| SyncError::IoError {
            path: path.display().to_string(),
            source: e,
        })?)
    }

    async fn is_external_symlink(&self, sync_root: &Path, full_path: &Path) -> Result<bool, SyncError> {
        let meta = tokio::fs::symlink_metadata(full_path).await.map_err(|e| SyncError::IoError {
            path: full_path.display().to_string(),
            source: e,
        })?;
        if !meta.file_type().is_symlink() {
            return Ok(false);
        }
        let sync_root_canon = sync_root.canonicalize().map_err(|e| SyncError::IoError {
            path: sync_root.display().to_string(),
            source: e,
        })?;
        let target = tokio::fs::read_link(full_path).await.map_err(|e| SyncError::IoError {
            path: full_path.display().to_string(),
            source: e,
        })?;
        Ok(!target.starts_with(&sync_root_canon))
    }
}
