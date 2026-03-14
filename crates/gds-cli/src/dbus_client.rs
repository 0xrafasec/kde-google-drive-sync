//! Session D-Bus client for `org.kde.GDriveSync.Daemon`.

use std::env;

use zbus::proxy;

use crate::dbus_types::{AccountInfo, QuotaInfo, SyncErrorInfo, SyncFolderInfo};

/// Default well-known name and object path on the session bus.
pub const DEFAULT_SERVICE: &str = "org.kde.GDriveSync";
pub const DEFAULT_PATH: &str = "/org/kde/GDriveSync";

pub const ENV_SERVICE: &str = "GDS_DBUS_SERVICE";
pub const ENV_PATH: &str = "GDS_DBUS_PATH";

#[derive(Clone, Debug)]
pub struct BusTarget {
    pub service: String,
    pub path: String,
}

impl Default for BusTarget {
    fn default() -> Self {
        Self::from_env()
    }
}

impl BusTarget {
    pub fn from_env() -> Self {
        Self {
            service: env::var(ENV_SERVICE).unwrap_or_else(|_| DEFAULT_SERVICE.to_string()),
            path: env::var(ENV_PATH).unwrap_or_else(|_| DEFAULT_PATH.to_string()),
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
    async fn add_account(&self) -> zbus::Result<()>;
    async fn remove_account(&self, id: &str) -> zbus::Result<()>;
    async fn get_sync_folders(&self) -> zbus::Result<Vec<SyncFolderInfo>>;
    async fn add_sync_folder(&self, local_path: &str, drive_folder_id: &str) -> zbus::Result<()>;
    async fn remove_sync_folder(&self, id: &str) -> zbus::Result<()>;
    async fn get_sync_errors(&self) -> zbus::Result<Vec<SyncErrorInfo>>;
    async fn get_about_info(&self, account_id: &str) -> zbus::Result<QuotaInfo>;
}

/// Holds session connection + target; proxies are built per call so the connection stays alive.
pub struct DaemonClient {
    conn: zbus::Connection,
    target: BusTarget,
}

impl DaemonClient {
    pub async fn connect(target: BusTarget) -> zbus::Result<Self> {
        let conn = zbus::Connection::session().await?;
        Ok(Self { conn, target })
    }

    pub async fn proxy(&self) -> zbus::Result<DaemonProxy<'_>> {
        DaemonProxy::builder(&self.conn)
            .destination(self.target.service.as_str())?
            .path(self.target.path.as_str())?
            .build()
            .await
    }

    pub fn target(&self) -> &BusTarget {
        &self.target
    }
}
