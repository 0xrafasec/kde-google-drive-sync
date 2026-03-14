//! Sync folder — mapping between local path and Drive folder.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A sync folder: local path ↔ Drive folder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncFolder {
    pub id: String,
    #[serde(rename = "accountId")]
    pub account_id: String,
    #[serde(rename = "localPath")]
    pub local_path: String,
    #[serde(rename = "driveFolderId")]
    pub drive_folder_id: String,
    /// Page token for changes.list (incremental sync). None until first sync.
    #[serde(default)]
    #[serde(rename = "startPageToken")]
    pub start_page_token: Option<String>,
    #[serde(default)]
    #[serde(rename = "lastSyncAt")]
    pub last_sync_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub paused: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_sync_folder() -> SyncFolder {
        SyncFolder {
            id: "sf-1".to_string(),
            account_id: "acc-1".to_string(),
            local_path: "/home/user/Drive".to_string(),
            drive_folder_id: "driveFolderId".to_string(),
            start_page_token: Some("token".to_string()),
            last_sync_at: None,
            paused: false,
        }
    }

    #[test]
    fn sync_folder_serialization_roundtrip() {
        let s = sample_sync_folder();
        let json = serde_json::to_string(&s).unwrap();
        let s2: SyncFolder = serde_json::from_str(&json).unwrap();
        assert_eq!(s, s2);
    }
}
