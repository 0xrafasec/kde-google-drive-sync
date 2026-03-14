//! Conflict information for user notification and resolution.

use serde::{Deserialize, Serialize};

/// Describes a sync conflict: local and server versions differ.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConflictInfo {
    #[serde(rename = "localPath")]
    pub local_path: String,
    #[serde(rename = "conflictCopyPath")]
    pub conflict_copy_path: String,
    #[serde(rename = "serverVersion")]
    pub server_version: String,
    #[serde(rename = "localVersion")]
    pub local_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conflict_info_serialization_roundtrip() {
        let c = ConflictInfo {
            local_path: "/home/user/Drive/file.txt".to_string(),
            conflict_copy_path: "/home/user/Drive/file.txt.conflict-20240115-120000".to_string(),
            server_version: "v2".to_string(),
            local_version: "v1".to_string(),
        };
        let json = serde_json::to_string(&c).unwrap();
        let c2: ConflictInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(c, c2);
    }
}
