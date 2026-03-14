//! D-Bus service interface (org.kde.GDriveSync).

mod service;
pub mod signals;
mod types;

pub use service::{DaemonService, DaemonState};
pub use types::{AccountInfo, QuotaInfo, SyncErrorInfo, SyncFolderInfo};
