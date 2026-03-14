//! Local file state — known state of a file for diff/sync.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::{SyncState, SyncStateKind};

/// Known state of a file within a sync folder (DB + Drive metadata).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileState {
    pub id: String,
    #[serde(rename = "syncFolderId")]
    pub sync_folder_id: String,
    #[serde(rename = "relativePath")]
    pub relative_path: String,
    #[serde(rename = "driveFileId")]
    pub drive_file_id: Option<String>,
    #[serde(default)]
    #[serde(rename = "driveMd5")]
    pub drive_md5: Option<String>,
    #[serde(default)]
    #[serde(rename = "driveModified")]
    pub drive_modified: Option<DateTime<Utc>>,
    #[serde(default)]
    #[serde(rename = "localMd5")]
    pub local_md5: Option<String>,
    #[serde(default)]
    #[serde(rename = "localModified")]
    pub local_modified: Option<DateTime<Utc>>,
    #[serde(rename = "syncState")]
    pub sync_state: SyncState,
    #[serde(default)]
    #[serde(rename = "lastSyncedAt")]
    pub last_synced_at: Option<DateTime<Utc>>,
}

impl FileState {
    /// Build a minimal file state (e.g. for new local file not yet on Drive).
    pub fn new_pending(id: String, sync_folder_id: String, relative_path: String) -> Self {
        Self {
            id,
            sync_folder_id,
            relative_path,
            drive_file_id: None,
            drive_md5: None,
            drive_modified: None,
            local_md5: None,
            local_modified: None,
            sync_state: SyncState::pending(),
            last_synced_at: None,
        }
    }

    /// Whether the file is in a terminal good state (no action needed).
    pub fn is_synced(&self) -> bool {
        self.sync_state.kind == SyncStateKind::Synced
    }

    /// Whether the file is in conflict.
    pub fn is_conflict(&self) -> bool {
        self.sync_state.kind == SyncStateKind::Conflict
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_state_new_pending() {
        let s = FileState::new_pending(
            "id1".to_string(),
            "sf1".to_string(),
            "path/to/file".to_string(),
        );
        assert_eq!(s.id, "id1");
        assert_eq!(s.sync_folder_id, "sf1");
        assert_eq!(s.relative_path, "path/to/file");
        assert!(s.drive_file_id.is_none());
        assert_eq!(s.sync_state.kind, SyncStateKind::Pending);
        assert!(!s.is_synced());
    }

    #[test]
    fn file_state_is_synced_and_is_conflict() {
        let mut s = FileState::new_pending("i".into(), "sf".into(), "p".into());
        s.sync_state = SyncState::synced();
        assert!(s.is_synced());
        assert!(!s.is_conflict());
        s.sync_state = SyncState::conflict();
        assert!(!s.is_synced());
        assert!(s.is_conflict());
    }

    #[test]
    fn file_state_serialization_roundtrip() {
        let s = FileState::new_pending("id".into(), "sf".into(), "a/b".into());
        let json = serde_json::to_string(&s).unwrap();
        let s2: FileState = serde_json::from_str(&json).unwrap();
        assert_eq!(s.id, s2.id);
        assert_eq!(s.relative_path, s2.relative_path);
    }
}
