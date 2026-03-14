//! D-Bus-friendly types for org.kde.GDriveSync.Daemon.
//! Use String and i64 (no Option) so zvariant::Type works.

use serde::{Deserialize, Serialize};
use zbus::zvariant::Type;

/// Account info (id, email, display_name) for GetAccounts.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct AccountInfo {
    pub id: String,
    pub email: String,
    pub display_name: String,
}

/// Sync folder info for GetSyncFolders. Empty string = None for optional fields; last_sync_at -1 = none.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct SyncFolderInfo {
    pub id: String,
    pub account_id: String,
    pub local_path: String,
    pub drive_folder_id: String,
    pub start_page_token: String,
    pub last_sync_at: i64,
    pub paused: bool,
}

/// Sync error entry for GetSyncErrors. file_state_id empty = none.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct SyncErrorInfo {
    pub id: String,
    pub file_state_id: String,
    pub error_message: String,
    pub occurred_at: i64,
    pub retry_count: i32,
}

/// Quota info from Drive about.get. Empty string = none.
#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct QuotaInfo {
    pub limit: String,
    pub usage: String,
    pub usage_in_drive: String,
}
