//! D-Bus wire types (must match `gds-daemon::dbus::types`).

use serde::{Deserialize, Serialize};
use zbus::zvariant::Type;

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct AccountInfo {
    pub id: String,
    pub email: String,
    pub display_name: String,
}

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

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct SyncErrorInfo {
    pub id: String,
    pub file_state_id: String,
    pub error_message: String,
    pub occurred_at: i64,
    pub retry_count: i32,
}

#[derive(Clone, Debug, Serialize, Deserialize, Type)]
pub struct QuotaInfo {
    pub limit: String,
    pub usage: String,
    pub usage_in_drive: String,
}
