//! Sync action types and change classification.

use chrono::{DateTime, Utc};

use crate::model::{DriveFile, FileState};

/// Kind of sync operation to perform.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SyncActionKind {
    /// New file on local only → upload to Drive.
    NewUpload,
    /// New file on Drive only → download to local.
    NewDownload,
    /// Local file changed since last sync → upload (update).
    UpdateUpload,
    /// Remote file changed since last sync → download (update).
    UpdateDownload,
    /// File removed locally but still on Drive → delete on Drive.
    DeleteRemote,
    /// File removed on Drive but still local → delete local.
    DeleteLocal,
    /// Both local and remote changed since last sync → server wins, keep local copy as conflict.
    Conflict,
}

/// A single sync action (what to do for one file).
#[derive(Clone, Debug)]
pub struct SyncAction {
    pub kind: SyncActionKind,
    pub relative_path: String,
    /// Existing file state from DB (if any).
    pub state: Option<FileState>,
    /// Drive file from changes/list (for downloads/conflict).
    pub drive_file: Option<DriveFile>,
    /// Local metadata when we have it (for uploads/conflict).
    pub local_md5: Option<String>,
    pub local_modified: Option<DateTime<Utc>>,
}

impl SyncAction {
    pub fn new_upload(relative_path: String, state: Option<FileState>, local_md5: String, local_modified: DateTime<Utc>) -> Self {
        Self {
            kind: SyncActionKind::NewUpload,
            relative_path,
            state,
            drive_file: None,
            local_md5: Some(local_md5),
            local_modified: Some(local_modified),
        }
    }

    pub fn new_download(relative_path: String, drive_file: DriveFile) -> Self {
        Self {
            kind: SyncActionKind::NewDownload,
            relative_path,
            state: None,
            drive_file: Some(drive_file),
            local_md5: None,
            local_modified: None,
        }
    }

    pub fn update_upload(
        relative_path: String,
        state: FileState,
        local_md5: String,
        local_modified: DateTime<Utc>,
    ) -> Self {
        Self {
            kind: SyncActionKind::UpdateUpload,
            relative_path,
            state: Some(state),
            drive_file: None,
            local_md5: Some(local_md5),
            local_modified: Some(local_modified),
        }
    }

    pub fn update_download(relative_path: String, state: FileState, drive_file: DriveFile) -> Self {
        Self {
            kind: SyncActionKind::UpdateDownload,
            relative_path,
            state: Some(state),
            drive_file: Some(drive_file),
            local_md5: None,
            local_modified: None,
        }
    }

    pub fn delete_remote(relative_path: String, state: FileState) -> Self {
        Self {
            kind: SyncActionKind::DeleteRemote,
            relative_path,
            state: Some(state),
            drive_file: None,
            local_md5: None,
            local_modified: None,
        }
    }

    pub fn delete_local(relative_path: String, state: FileState) -> Self {
        Self {
            kind: SyncActionKind::DeleteLocal,
            relative_path,
            state: Some(state),
            drive_file: None,
            local_md5: None,
            local_modified: None,
        }
    }

    pub fn conflict(
        relative_path: String,
        state: FileState,
        drive_file: DriveFile,
        local_md5: String,
        local_modified: DateTime<Utc>,
    ) -> Self {
        Self {
            kind: SyncActionKind::Conflict,
            relative_path,
            state: Some(state),
            drive_file: Some(drive_file),
            local_md5: Some(local_md5),
            local_modified: Some(local_modified),
        }
    }

    /// Priority for queue: downloads before uploads for initial sync.
    pub fn priority(&self) -> u8 {
        match self.kind {
            SyncActionKind::NewDownload | SyncActionKind::UpdateDownload => 0,
            SyncActionKind::Conflict => 1,
            SyncActionKind::NewUpload | SyncActionKind::UpdateUpload => 2,
            SyncActionKind::DeleteLocal | SyncActionKind::DeleteRemote => 3,
        }
    }
}

/// Classifies whether we have conflict (both sides changed since last sync).
#[allow(dead_code)]
pub fn is_conflict(
    state: &FileState,
    local_md5: &str,
    local_modified: DateTime<Utc>,
    drive_md5: Option<&str>,
    drive_modified: Option<DateTime<Utc>>,
) -> bool {
    let local_changed = state.local_md5.as_deref() != Some(local_md5)
        || state.local_modified.map(|t| t != local_modified).unwrap_or(true);
    let remote_changed = state.drive_md5.as_deref() != drive_md5
        || state.drive_modified != drive_modified;
    local_changed && remote_changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::SyncState;

    fn state_with(local_md5: Option<&str>, local_mod: Option<DateTime<Utc>>, drive_md5: Option<&str>, drive_mod: Option<DateTime<Utc>>) -> FileState {
        let t = Utc::now();
        FileState {
            id: "id".into(),
            sync_folder_id: "sf".into(),
            relative_path: "f".into(),
            drive_file_id: Some("did".into()),
            drive_md5: drive_md5.map(String::from),
            drive_modified: drive_mod,
            local_md5: local_md5.map(String::from),
            local_modified: local_mod,
            sync_state: SyncState::synced(),
            last_synced_at: Some(t),
        }
    }

    #[test]
    fn conflict_both_changed() {
        let t = Utc::now();
        let state = state_with(Some("m1"), Some(t), Some("d1"), Some(t));
        assert!(is_conflict(&state, "m2", t, Some("d2"), Some(t)));
    }

    #[test]
    fn no_conflict_only_local_changed() {
        let t = Utc::now();
        let state = state_with(Some("m1"), Some(t), Some("d1"), Some(t));
        assert!(!is_conflict(&state, "m2", t, Some("d1"), Some(t)));
    }

    #[test]
    fn no_conflict_only_remote_changed() {
        let t = Utc::now();
        let state = state_with(Some("m1"), Some(t), Some("d1"), Some(t));
        assert!(!is_conflict(&state, "m1", t, Some("d2"), Some(t)));
    }

    #[test]
    fn no_conflict_neither_changed() {
        let t = Utc::now();
        let state = state_with(Some("m1"), Some(t), Some("d1"), Some(t));
        assert!(!is_conflict(&state, "m1", t, Some("d1"), Some(t)));
    }
}
