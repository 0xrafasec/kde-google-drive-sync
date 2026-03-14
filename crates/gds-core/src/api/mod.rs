//! Google Drive API v3 client.
//!
//! All methods take an `access_token`; token refresh is handled by the auth layer (see crate::auth).

mod backoff;
mod client;
mod error;
mod types;
mod workspace;

pub use client::{
    DriveClient, CHANGES_FIELDS, DEFAULT_BASE_URL, FILE_FIELDS, RESUMABLE_CHUNK_SIZE,
    SIMPLE_UPLOAD_MAX_BYTES,
};
pub use error::{retry_after_seconds, status_to_sync_error};
pub use types::{
    AboutResponse, CreateFileMetadata, DriveListResponse, FileListResponse, UpdateFileMetadata,
};
pub use workspace::{export_mime_type, is_google_workspace_file};
