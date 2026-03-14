//! Session D-Bus client for daemon (`org.kde.GDriveSync.Daemon`).

use std::env;

use serde::{Deserialize, Serialize};
use zbus::proxy;
use zbus::zvariant::Type;

pub const DEFAULT_SERVICE: &str = "org.kde.GDriveSync";
pub const DEFAULT_PATH: &str = "/org/kde/GDriveSync";

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

#[derive(Clone, Debug)]
pub struct BusTarget {
    pub service: String,
    pub path: String,
}

impl Default for BusTarget {
    fn default() -> Self {
        Self {
            service: env::var("GDS_DBUS_SERVICE").unwrap_or_else(|_| DEFAULT_SERVICE.to_string()),
            path: env::var("GDS_DBUS_PATH").unwrap_or_else(|_| DEFAULT_PATH.to_string()),
        }
    }
}

#[proxy(interface = "org.kde.GDriveSync.Daemon", assume_defaults = false)]
pub trait Daemon {
    async fn get_status(&self) -> zbus::Result<(String, u32)>;
    async fn pause_sync(&self) -> zbus::Result<()>;
    async fn resume_sync(&self) -> zbus::Result<()>;
    async fn force_sync(&self, path: &str) -> zbus::Result<()>;
    async fn get_accounts(&self) -> zbus::Result<Vec<AccountInfo>>;
    async fn get_sync_folders(&self) -> zbus::Result<Vec<SyncFolderInfo>>;
    async fn get_about_info(&self, account_id: &str)
        -> zbus::Result<(String, String, String)>;
}

pub async fn connect_session() -> zbus::Result<zbus::Connection> {
    zbus::Connection::session().await
}

pub async fn daemon_proxy<'a>(
    conn: &'a zbus::Connection,
    target: &'a BusTarget,
) -> zbus::Result<DaemonProxy<'a>> {
    DaemonProxy::builder(conn)
        .destination(target.service.as_str())?
        .path(target.path.as_str())?
        .build()
        .await
}
