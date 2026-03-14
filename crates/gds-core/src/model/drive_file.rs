//! Drive file metadata (Drive API v3 file resource).

use serde::{Deserialize, Serialize};

/// Drive file resource as returned by Drive API v3 (partial response).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveFile {
    pub id: String,
    pub name: String,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    #[serde(default)]
    #[serde(rename = "md5Checksum")]
    pub md5_checksum: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    #[serde(rename = "modifiedTime")]
    pub modified_time: Option<String>,
    #[serde(default)]
    pub parents: Option<Vec<String>>,
    #[serde(default)]
    pub trashed: Option<bool>,
}

impl DriveFile {
    /// Size in bytes, if present and parseable.
    pub fn size_bytes(&self) -> Option<u64> {
        self.size.as_ref().and_then(|s| s.parse().ok())
    }

    /// Whether the file is in trash.
    pub fn is_trashed(&self) -> bool {
        self.trashed.unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_file() -> DriveFile {
        DriveFile {
            id: "abc123".to_string(),
            name: "doc.pdf".to_string(),
            mime_type: "application/pdf".to_string(),
            md5_checksum: Some("d41d8cd98f00b204e9800998ecf8427e".to_string()),
            size: Some("1024".to_string()),
            modified_time: Some("2024-01-15T12:00:00.000Z".to_string()),
            parents: Some(vec!["folderId".to_string()]),
            trashed: Some(false),
        }
    }

    #[test]
    fn drive_file_size_bytes() {
        let f = sample_file();
        assert_eq!(f.size_bytes(), Some(1024));
        let f2 = DriveFile { size: Some("invalid".to_string()), ..sample_file() };
        assert_eq!(f2.size_bytes(), None);
    }

    #[test]
    fn drive_file_is_trashed() {
        let f = sample_file();
        assert!(!f.is_trashed());
        let f2 = DriveFile { trashed: Some(true), ..sample_file() };
        assert!(f2.is_trashed());
    }

    #[test]
    fn drive_file_serialization_roundtrip() {
        let f = sample_file();
        let json = serde_json::to_string(&f).unwrap();
        let f2: DriveFile = serde_json::from_str(&json).unwrap();
        assert_eq!(f, f2);
    }
}
