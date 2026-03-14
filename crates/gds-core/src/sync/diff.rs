//! Diff engine: compare local, DB, and Drive changes to produce sync actions.

use std::collections::HashMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use tracing::instrument;

use crate::db::FileStateRepository;
use crate::model::{ChangeSet, FileState, SyncFolder};
use crate::sync::change::{SyncAction, SyncActionKind};
use crate::sync::path::safe_local_path;
use crate::sync::fs::LocalFs;
use crate::model::SyncError;

/// Parses Drive API modifiedTime (RFC3339) to DateTime<Utc>.
pub fn parse_drive_modified(s: Option<&str>) -> Option<DateTime<Utc>> {
    let s = s?;
    DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.with_timezone(&Utc))
}

/// Diff engine: computes local and remote changes, classifies actions.
pub struct DiffEngine;

impl DiffEngine {
    /// Walk local directory (one level at a time to avoid OOM), compare to DB, produce actions.
    /// Skips .gds_tmp and paths under .git.
    #[instrument(skip(pool, local_fs), level = "debug")]
    pub async fn compute_local_changes(
        sync_root: &Path,
        sync_folder: &SyncFolder,
        pool: &SqlitePool,
        local_fs: &dyn LocalFs,
    ) -> Result<Vec<SyncAction>, SyncError> {
        let sync_folder_id = &sync_folder.id;
        let mut actions = Vec::new();
        let mut dir_queue: Vec<String> = vec![String::new()];
        let db_states = FileStateRepository::list_by_folder(pool, sync_folder_id).await
            .map_err(|e| SyncError::DatabaseError(Box::new(e)))?;
        let by_path: HashMap<String, FileState> = db_states.into_iter().map(|s| (s.relative_path.clone(), s)).collect();

        while let Some(rel_dir) = dir_queue.pop() {
            let entries = local_fs.list_dir(sync_root, &rel_dir).await?;
            for entry in entries {
                if entry.name == ".gds_tmp" || entry.name == ".git" {
                    continue;
                }
                let relative_path = if rel_dir.is_empty() {
                    entry.name.clone()
                } else {
                    format!("{}/{}", rel_dir, entry.name)
                };
                if relative_path.contains("/.git/") {
                    continue;
                }

                let _ = safe_local_path(sync_root, &relative_path).map_err(|e| {
                    tracing::warn!(path = %relative_path, "path validation failed: {}", e);
                    e
                })?;

                if entry.is_dir {
                    dir_queue.push(relative_path);
                    continue;
                }

                let meta = match local_fs.file_metadata(sync_root, &relative_path).await? {
                    Some(m) => m,
                    None => continue,
                };

                let state = by_path.get(&relative_path);
                if let Some(state) = state {
                    let local_changed = state.local_md5.as_deref() != Some(&meta.md5)
                        || state.local_modified.map(|t| t != meta.modified).unwrap_or(true);
                    if local_changed {
                        actions.push(SyncAction::update_upload(
                            relative_path,
                            state.clone(),
                            meta.md5,
                            meta.modified,
                        ));
                    }
                } else {
                    actions.push(SyncAction::new_upload(
                        relative_path,
                        None,
                        meta.md5,
                        meta.modified,
                    ));
                }
            }
        }

        for (relative_path, state) in &by_path {
            if state.drive_file_id.is_none() {
                continue;
            }
            let exists = local_fs.exists(sync_root, relative_path).await?;
            if !exists {
                actions.push(SyncAction::delete_remote(relative_path.clone(), state.clone()));
            }
        }

        Ok(actions)
    }

    /// Process Drive changes.list result, compare to DB, produce actions.
    #[instrument(skip(pool), level = "debug")]
    pub async fn compute_remote_changes(
        sync_folder: &SyncFolder,
        change_set: &ChangeSet,
        pool: &SqlitePool,
    ) -> Result<Vec<SyncAction>, SyncError> {
        let sync_folder_id = &sync_folder.id;
        let drive_folder_id = &sync_folder.drive_folder_id;
        let db_states = FileStateRepository::list_by_folder(pool, sync_folder_id).await
            .map_err(|e| SyncError::DatabaseError(Box::new(e)))?;
        let by_drive_id: HashMap<String, FileState> = db_states
            .iter()
            .filter_map(|s| s.drive_file_id.as_ref().map(|id| (id.clone(), s.clone())))
            .collect();

        let mut drive_id_to_path: HashMap<String, String> = by_drive_id.iter().map(|(id, s)| (id.clone(), s.relative_path.clone())).collect();

        let mut actions = Vec::new();
        for change in &change_set.changes {
            let file_id = &change.file_id;
            if change.removed == Some(true) {
                if let Some(state) = by_drive_id.get(file_id) {
                    actions.push(SyncAction::delete_local(state.relative_path.clone(), state.clone()));
                }
                continue;
            }

            let drive_file = match &change.file {
                Some(f) if !f.is_trashed() => f.clone(),
                _ => continue,
            };

            let parent_id = drive_file.parents.as_deref().and_then(|p| p.first()).map(String::as_str);
            let relative_path = if let Some(state) = by_drive_id.get(file_id) {
                state.relative_path.clone()
            } else {
                let parent_path = parent_id
                    .and_then(|pid| drive_id_to_path.get(pid).cloned())
                    .unwrap_or_else(|| String::new());
                if parent_path.is_empty() && parent_id != Some(drive_folder_id.as_str()) {
                    continue;
                }
                let rel = if parent_path.is_empty() {
                    drive_file.name.clone()
                } else {
                    format!("{}/{}", parent_path, drive_file.name)
                };
                drive_id_to_path.insert(file_id.clone(), rel.clone());
                rel
            };

            let drive_md5 = drive_file.md5_checksum.as_deref();
            let drive_modified = parse_drive_modified(drive_file.modified_time.as_deref());

            let state = FileStateRepository::get_by_path(pool, sync_folder_id, &relative_path).await
                .map_err(|e| SyncError::DatabaseError(Box::new(e)))?
                .or_else(|| by_drive_id.get(file_id).cloned());

            if let Some(state) = state {
                let remote_changed = state.drive_md5.as_deref() != drive_md5
                    || state.drive_modified != drive_modified;

                if !remote_changed {
                    continue;
                }

                actions.push(SyncAction::update_download(relative_path, state, drive_file));
            } else {
                actions.push(SyncAction::new_download(relative_path, drive_file));
            }
        }

        Ok(actions)
    }

    /// Merges local and remote action lists. When the same path has both UpdateUpload and UpdateDownload, produces Conflict (server wins).
    pub fn merge_actions(
        local_actions: Vec<SyncAction>,
        remote_actions: Vec<SyncAction>,
    ) -> Vec<SyncAction> {
        let mut by_path: HashMap<String, SyncAction> = HashMap::new();
        for a in local_actions {
            by_path.insert(a.relative_path.clone(), a);
        }
        for mut a in remote_actions {
            let path = a.relative_path.clone();
            if let Some(local) = by_path.remove(&path) {
                match (&local.kind, &a.kind) {
                    (SyncActionKind::UpdateUpload, SyncActionKind::UpdateDownload) => {
                        if let (Some(state), Some(drive_file), Some(local_md5), Some(local_modified)) = (
                            local.state.clone(),
                            a.drive_file.clone(),
                            local.local_md5.clone(),
                            local.local_modified,
                        ) {
                            a = SyncAction::conflict(
                                path.clone(),
                                state,
                                drive_file,
                                local_md5,
                                local_modified,
                            );
                        } else {
                            by_path.insert(path.clone(), local);
                            by_path.insert(path, a);
                            continue;
                        }
                    }
                    (SyncActionKind::NewUpload, SyncActionKind::NewDownload) => {
                        by_path.insert(path.clone(), local);
                        by_path.insert(path, a);
                        continue;
                    }
                    _ => {
                        by_path.insert(path.clone(), local);
                    }
                }
            }
            by_path.insert(path, a);
        }
        by_path.into_values().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_drive_modified_some() {
        let t = parse_drive_modified(Some("2024-01-15T12:00:00.000Z"));
        assert!(t.is_some());
    }

    #[test]
    fn parse_drive_modified_none() {
        assert!(parse_drive_modified(None).is_none());
        assert!(parse_drive_modified(Some("invalid")).is_none());
    }
}
