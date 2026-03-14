//! Change set from Drive API changes.list.

use serde::{Deserialize, Serialize};

use super::DriveFile;

/// A single change from changes.list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveChange {
    #[serde(rename = "changeType")]
    pub change_type: String,
    #[serde(rename = "fileId")]
    pub file_id: String,
    #[serde(default)]
    pub file: Option<DriveFile>,
    #[serde(default)]
    #[serde(rename = "removed")]
    pub removed: Option<bool>,
}

/// Paginated response from changes.list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChangeSet {
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
    #[serde(rename = "newStartPageToken")]
    pub new_start_page_token: Option<String>,
    pub changes: Vec<DriveChange>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DriveFile;

    fn sample_change_set() -> ChangeSet {
        ChangeSet {
            next_page_token: Some("next".to_string()),
            new_start_page_token: None,
            changes: vec![
                DriveChange {
                    change_type: "file".to_string(),
                    file_id: "f1".to_string(),
                    file: Some(DriveFile {
                        id: "f1".to_string(),
                        name: "a.txt".to_string(),
                        mime_type: "text/plain".to_string(),
                        md5_checksum: None,
                        size: None,
                        modified_time: None,
                        parents: None,
                        trashed: None,
                    }),
                    removed: Some(false),
                },
            ],
        }
    }

    #[test]
    fn change_set_serialization_roundtrip() {
        let c = sample_change_set();
        let json = serde_json::to_string(&c).unwrap();
        let c2: ChangeSet = serde_json::from_str(&json).unwrap();
        assert_eq!(c.next_page_token, c2.next_page_token);
        assert_eq!(c.changes.len(), c2.changes.len());
    }
}
