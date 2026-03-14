//! D-Bus service implementation (org.kde.GDriveSync.Daemon).

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use chrono::Utc;
use gds_core::api::DriveClient;
use gds_core::auth::{authorize_flow, TokenProvider, TokenStore};
use gds_core::db::{AccountRepository, SyncErrorRepository, SyncFolderRepository};
use gds_core::model::{Account, Config, SyncFolder};
use gds_core::sync::safe_local_path;
use sqlx::SqlitePool;
use tokio::sync::mpsc;
use tracing::{info, warn};
use zbus::interface;
use zbus::object_server::SignalEmitter;

use super::types::{AccountInfo, QuotaInfo, SyncErrorInfo, SyncFolderInfo};
use crate::scheduler::SyncRequest;

/// Shared state for the D-Bus service (and scheduler).
pub struct DaemonState {
    pub pool: SqlitePool,
    pub config: Config,
    pub token_provider: Arc<TokenProvider>,
    pub token_store: Arc<dyn TokenStore>,
    pub drive_client: DriveClient,
    pub pause: Arc<AtomicBool>,
    pub syncing_count: Arc<AtomicU32>,
    pub sync_request_tx: mpsc::UnboundedSender<SyncRequest>,
}

/// D-Bus service implementation.
pub struct DaemonService {
    pub state: Arc<DaemonState>,
}

#[interface(name = "org.kde.GDriveSync.Daemon")]
impl DaemonService {
    async fn get_status(&self) -> zbus::fdo::Result<(String, u32)> {
        let paused = self.state.pause.load(Ordering::Relaxed);
        let count = self.state.syncing_count.load(Ordering::Relaxed);
        let status = if paused {
            "paused"
        } else if count > 0 {
            "syncing"
        } else {
            "idle"
        };
        Ok((status.to_string(), count))
    }

    async fn pause_sync(&self) -> zbus::fdo::Result<()> {
        self.state.pause.store(true, Ordering::Relaxed);
        let folders = list_all_folders(&self.state.pool)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("list folders: {}", e)))?;
        for f in folders {
            if let Err(e) = SyncFolderRepository::set_paused(&self.state.pool, &f.id, true).await {
                warn!(folder_id = %f.id, error = %e, "set_paused failed");
            }
        }
        Ok(())
    }

    async fn resume_sync(&self) -> zbus::fdo::Result<()> {
        self.state.pause.store(false, Ordering::Relaxed);
        let folders = list_all_folders(&self.state.pool)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("list folders: {}", e)))?;
        for f in folders {
            if let Err(e) = SyncFolderRepository::set_paused(&self.state.pool, &f.id, false).await {
                warn!(folder_id = %f.id, error = %e, "set_paused failed");
            }
        }
        Ok(())
    }

    async fn force_sync(&self, path: &str) -> zbus::fdo::Result<()> {
        let folders = list_all_folders(&self.state.pool)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("list folders: {}", e)))?;
        for f in &folders {
            if path.is_empty() || path == f.local_path || f.local_path.starts_with(path) {
                self.state
                    .sync_request_tx
                    .send(SyncRequest {
                        sync_folder_id: f.id.clone(),
                    })
                    .map_err(|e| zbus::fdo::Error::Failed(format!("send sync request: {}", e)))?;
                return Ok(());
            }
        }
        if path.is_empty() {
            for f in folders {
                let _ = self.state.sync_request_tx.send(SyncRequest {
                    sync_folder_id: f.id,
                });
            }
        }
        Ok(())
    }

    async fn get_accounts(&self) -> zbus::fdo::Result<Vec<AccountInfo>> {
        let accounts = AccountRepository::list_all(&self.state.pool)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("list accounts: {}", e)))?;
        Ok(accounts
            .into_iter()
            .map(|a| AccountInfo {
                id: a.id,
                email: a.email,
                display_name: a.display_name.unwrap_or_default(),
            })
            .collect())
    }

    async fn add_account(&self) -> zbus::fdo::Result<()> {
        let account_id = uuid::Uuid::new_v4().to_string();
        let keyring_key = format!("gds:{}", account_id);
        let store = self.state.token_store.as_ref();
        let client_id = self.state.config.oauth.client_id.clone();
        let client_secret: Option<String> = None;
        let redirect_port = self.state.config.oauth.redirect_port;

        let open_url = |url: &str| {
            let _ = std::process::Command::new("xdg-open").arg(url).output();
            Ok(())
        };

        authorize_flow(
            &client_id,
            client_secret.as_deref(),
            redirect_port,
            store,
            &keyring_key,
            Some(open_url),
        )
        .await
        .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        let account = Account {
            id: account_id.clone(),
            email: String::new(),
            display_name: None,
            keyring_key: keyring_key.clone(),
            created_at: Utc::now(),
        };
        AccountRepository::insert(&self.state.pool, &account)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("insert account: {}", e)))?;
        info!(account_id = %account_id, "account added");
        Ok(())
    }

    async fn remove_account(&self, id: &str) -> zbus::fdo::Result<()> {
        let account = AccountRepository::get_by_id(&self.state.pool, id)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("get account: {}", e)))?
            .ok_or_else(|| zbus::fdo::Error::Failed("account not found".to_string()))?;

        self.state
            .token_provider
            .revoke_and_remove(&account.keyring_key)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        AccountRepository::delete_cascade(&self.state.pool, id)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("delete account: {}", e)))?;
        info!(account_id = %id, "account removed");
        Ok(())
    }

    async fn get_sync_folders(&self) -> zbus::fdo::Result<Vec<SyncFolderInfo>> {
        let folders = list_all_folders(&self.state.pool)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("list folders: {}", e)))?;
        Ok(folders
            .into_iter()
            .map(|f| SyncFolderInfo {
                id: f.id,
                account_id: f.account_id,
                local_path: f.local_path,
                drive_folder_id: f.drive_folder_id,
                start_page_token: f.start_page_token.unwrap_or_default(),
                last_sync_at: f.last_sync_at.map(|t| t.timestamp()).unwrap_or(-1),
                paused: f.paused,
            })
            .collect())
    }

    async fn add_sync_folder(
        &self,
        local_path: &str,
        drive_folder_id: &str,
    ) -> zbus::fdo::Result<()> {
        let sync_root = Path::new(local_path);
        safe_local_path(sync_root, "")
            .map_err(|e| zbus::fdo::Error::Failed(format!("invalid path: {}", e)))?;

        let account_ids: Vec<String> = AccountRepository::list_all(&self.state.pool)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("list accounts: {}", e)))?
            .into_iter()
            .map(|a| a.id)
            .collect();
        let account_id = account_ids.first().ok_or_else(|| {
            zbus::fdo::Error::Failed("no account; add an account first".to_string())
        })?;

        let folder = SyncFolder {
            id: uuid::Uuid::new_v4().to_string(),
            account_id: account_id.clone(),
            local_path: local_path.to_string(),
            drive_folder_id: drive_folder_id.to_string(),
            start_page_token: None,
            last_sync_at: None,
            paused: false,
        };
        SyncFolderRepository::insert(&self.state.pool, &folder)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("insert folder: {}", e)))?;
        let _ = self.state.sync_request_tx.send(SyncRequest {
            sync_folder_id: folder.id.clone(),
        });
        Ok(())
    }

    async fn remove_sync_folder(&self, id: &str) -> zbus::fdo::Result<()> {
        SyncFolderRepository::delete(&self.state.pool, id)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("delete folder: {}", e)))?;
        Ok(())
    }

    async fn get_sync_errors(&self) -> zbus::fdo::Result<Vec<SyncErrorInfo>> {
        let limit = 100i64;
        let records = SyncErrorRepository::get_recent(&self.state.pool, None, limit)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("get errors: {}", e)))?;
        Ok(records
            .into_iter()
            .map(|r| SyncErrorInfo {
                id: r.id,
                file_state_id: r.file_state_id.unwrap_or_default(),
                error_message: r.error_message,
                occurred_at: r.occurred_at.timestamp(),
                retry_count: r.retry_count,
            })
            .collect())
    }

    async fn get_about_info(&self, account_id: &str) -> zbus::fdo::Result<QuotaInfo> {
        let account = AccountRepository::get_by_id(&self.state.pool, account_id)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(format!("get account: {}", e)))?
            .ok_or_else(|| zbus::fdo::Error::Failed("account not found".to_string()))?;

        let token = self
            .state
            .token_provider
            .get_valid_access_token(&account.keyring_key)
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        let about = self
            .state
            .drive_client
            .about_get(&token, "user,storageQuota")
            .await
            .map_err(|e| zbus::fdo::Error::Failed(e.to_string()))?;

        Ok(about
            .storage_quota
            .map(|q| QuotaInfo {
                limit: q.limit.unwrap_or_default(),
                usage: q.usage.unwrap_or_default(),
                usage_in_drive: q.usage_in_drive.unwrap_or_default(),
            })
            .unwrap_or(QuotaInfo {
                limit: String::new(),
                usage: String::new(),
                usage_in_drive: String::new(),
            }))
    }

    #[zbus(signal)]
    async fn sync_started(
        emitter: &SignalEmitter<'_>,
        account_id: &str,
        path: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn sync_completed(
        emitter: &SignalEmitter<'_>,
        account_id: &str,
        path: &str,
        files_synced: u32,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn sync_error(
        emitter: &SignalEmitter<'_>,
        account_id: &str,
        path: &str,
        error: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn conflict_detected(
        emitter: &SignalEmitter<'_>,
        local_path: &str,
        conflict_copy: &str,
    ) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn status_changed(emitter: &SignalEmitter<'_>, new_status: &str) -> zbus::Result<()>;
}

async fn list_all_folders(pool: &SqlitePool) -> Result<Vec<SyncFolder>, sqlx::Error> {
    let accounts = AccountRepository::list_all(pool).await?;
    let mut folders = Vec::new();
    for a in accounts {
        let f = SyncFolderRepository::list_by_account(pool, &a.id).await?;
        folders.extend(f);
    }
    Ok(folders)
}
