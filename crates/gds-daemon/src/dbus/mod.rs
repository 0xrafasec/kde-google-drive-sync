//! D-Bus service interface (org.kde.GDriveSync).

mod service;
mod types;

pub use service::{DaemonService, DaemonState};
pub use types::{AccountInfo, QuotaInfo, SyncErrorInfo, SyncFolderInfo};
