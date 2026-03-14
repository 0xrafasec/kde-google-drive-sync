//! Request/response types for Drive API v3 (minimal, partial response–friendly).

use serde::{Deserialize, Serialize};

use crate::model::DriveFile;

/// Response from `files.list`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileListResponse {
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
    #[serde(default)]
    pub files: Vec<DriveFile>,
}

/// Metadata for `files.create` (and resumable init).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CreateFileMetadata {
    pub name: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
    pub parents: Option<Vec<String>>,
}

/// Metadata for `files.update` (patch: only set fields that change).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct UpdateFileMetadata {
    pub name: Option<String>,
    pub parents: Option<Vec<String>>,
    pub trashed: Option<bool>,
}

/// Response from `about.get` (quota, user info). Partial response.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AboutResponse {
    #[serde(rename = "user")]
    pub user: Option<AboutUser>,
    #[serde(rename = "storageQuota")]
    pub storage_quota: Option<StorageQuota>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AboutUser {
    #[serde(rename = "displayName")]
    pub display_name: Option<String>,
    #[serde(rename = "emailAddress")]
    pub email_address: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageQuota {
    pub limit: Option<String>,
    pub usage: Option<String>,
    #[serde(rename = "usageInDrive")]
    pub usage_in_drive: Option<String>,
}

/// Stub for `drive.list` (shared drives). Returns empty list for now.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveListResponse {
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
    #[serde(default)]
    pub drives: Vec<DriveInfo>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DriveInfo {
    pub id: String,
    pub name: Option<String>,
}
