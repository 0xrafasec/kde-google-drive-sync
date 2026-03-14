//! Scheduler: poll + event-driven sync, rate limiting, retry backoff.

mod rate_limit;
mod retry;

use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use gds_core::api::{DriveClient, CHANGES_FIELDS};
use gds_core::auth::TokenProvider;
use gds_core::db::{AccountRepository, SyncErrorRepository, SyncFolderRepository};
use gds_core::model::{ChangeSet, Config, SyncError, SyncFolder};
use gds_core::sync::{DiffEngine, SyncExecutor, SyncQueue, TokioLocalFs};
use sqlx::SqlitePool;
use tokio::sync::mpsc;
use tracing::{debug, error, info, instrument, warn};
use zbus::Connection;

use crate::dbus::signals;

pub use rate_limit::TokenBucket;
pub use retry::{backoff_duration, next_retry_at, should_retry_now};

/// Request to run sync for a folder (from file watcher or D-Bus ForceSync).
#[derive(Clone, Debug)]
pub struct SyncRequest {
    pub sync_folder_id: String,
}

/// Runs one full sync cycle for a folder: changes list, diff, merge, execute.
#[instrument(skip(pool, drive_client, token_provider, pause, conn), level = "info")]
pub async fn run_sync_loop(
    pool: &SqlitePool,
    drive_client: &DriveClient,
    token_provider: Arc<TokenProvider>,
    config: &Config,
    sync_folder: &SyncFolder,
    pause: &AtomicBool,
    conn: &Connection,
    syncing_count: &AtomicU32,
) -> Result<u32, SyncError> {
    let account = AccountRepository::get_by_id(pool, &sync_folder.account_id)
        .await
        .map_err(|e| SyncError::DatabaseError(Box::new(e)))?
        .ok_or_else(|| SyncError::AuthError {
            message: format!("account {} not found", sync_folder.account_id),
        })?;

    let token = token_provider
        .get_valid_access_token(&account.keyring_key)
        .await?;

    let mut folder = sync_folder.clone();

    if folder.start_page_token.is_none() {
        let start = drive_client
            .changes_get_start_page_token(&token, None)
            .await?;
        folder.start_page_token = Some(start.clone());
        SyncFolderRepository::update_page_token(pool, &folder.id, Some(&start))
            .await
            .map_err(|e| SyncError::DatabaseError(Box::new(e)))?;
    }

    let page_token = folder
        .start_page_token
        .as_deref()
        .ok_or_else(|| SyncError::ApiError {
            code: 0,
            message: "missing start_page_token".to_string(),
        })?;

    let merged = fetch_all_changes(drive_client, &token, page_token).await?;

    let sync_root = Path::new(&folder.local_path);
    let local_fs = Arc::new(TokioLocalFs);

    let local_actions =
        DiffEngine::compute_local_changes(sync_root, &folder, pool, local_fs.as_ref()).await?;

    let remote_actions = DiffEngine::compute_remote_changes(&folder, &merged, pool).await?;
    let actions = DiffEngine::merge_actions(local_actions, remote_actions);
    let mut queue = SyncQueue::from_actions(actions);

    let (conflict_tx, mut conflict_rx) = tokio::sync::mpsc::unbounded_channel::<(String, String)>();
    let conn_conf = conn.clone();
    let conflict_task = tokio::spawn(async move {
        while let Some((local_path, conflict_copy)) = conflict_rx.recv().await {
            if signals::conflict_detected(&conn_conf, &local_path, &conflict_copy)
                .await
                .is_err()
            {
                break;
            }
        }
    });

    syncing_count.fetch_add(1, Ordering::Relaxed);
    let _ = signals::sync_started(conn, &folder.account_id, &folder.local_path).await;
    let _ = signals::status_changed(conn, "syncing").await;

    let executor = SyncExecutor::new(
        drive_client.clone(),
        token_provider,
        account.keyring_key.clone(),
        local_fs,
        pool.clone(),
        config.clone(),
    )
    .with_conflict_notifier(conflict_tx);

    let executed = executor.run(&folder, &mut queue, pause).await;

    let executed = match executed {
        Ok(n) => n,
        Err(e) => {
            syncing_count.fetch_sub(1, Ordering::Relaxed);
            let status = if pause.load(Ordering::Relaxed) {
                "paused"
            } else {
                "idle"
            };
            let _ = signals::status_changed(conn, status).await;
            let _ = signals::sync_error(
                conn,
                &folder.account_id,
                &folder.local_path,
                &e.to_string(),
            )
            .await;
            conflict_task.abort();
            return Err(e);
        }
    };

    syncing_count.fetch_sub(1, Ordering::Relaxed);
    let status = if pause.load(Ordering::Relaxed) {
        "paused"
    } else {
        "idle"
    };
    let _ = signals::sync_completed(
        conn,
        &folder.account_id,
        &folder.local_path,
        executed,
    )
    .await;
    let _ = signals::status_changed(conn, status).await;

    if let Some(new_token) = &merged.new_start_page_token {
        SyncFolderRepository::update_page_token(pool, &folder.id, Some(new_token))
            .await
            .map_err(|e| SyncError::DatabaseError(Box::new(e)))?;
    }

    Ok(executed)
}

async fn fetch_all_changes(
    drive_client: &DriveClient,
    token: &str,
    start_page_token: &str,
) -> Result<ChangeSet, SyncError> {
    let mut all_changes = Vec::new();
    let mut page_token = start_page_token.to_string();
    let mut new_start_page_token = None::<String>;

    loop {
        let set = drive_client
            .changes_list(token, &page_token, None, CHANGES_FIELDS, false, false)
            .await?;
        all_changes.extend(set.changes);
        if set.new_start_page_token.is_some() {
            new_start_page_token = set.new_start_page_token;
        }
        match set.next_page_token {
            Some(next) => page_token = next,
            None => break,
        }
    }

    Ok(ChangeSet {
        next_page_token: None,
        new_start_page_token,
        changes: all_changes,
    })
}

/// Scheduler: receives sync requests, rate-limits, runs sync loop per folder.
pub struct Scheduler {
    pool: SqlitePool,
    drive_client: DriveClient,
    token_provider: Arc<TokenProvider>,
    config: Config,
    pause: Arc<AtomicBool>,
    rate_limiter: Arc<TokenBucket>,
    /// Receiver for sync requests (folder id).
    rx: mpsc::UnboundedReceiver<SyncRequest>,
    /// Shutdown signal
    shutdown: Arc<AtomicBool>,
    conn: Arc<Connection>,
    syncing_count: Arc<AtomicU32>,
}

impl Scheduler {
    pub fn new(
        pool: SqlitePool,
        drive_client: DriveClient,
        token_provider: Arc<TokenProvider>,
        config: Config,
        pause: Arc<AtomicBool>,
        rate_limiter: Arc<TokenBucket>,
        rx: mpsc::UnboundedReceiver<SyncRequest>,
        shutdown: Arc<AtomicBool>,
        conn: Arc<Connection>,
        syncing_count: Arc<AtomicU32>,
    ) -> Self {
        Self {
            pool,
            drive_client,
            token_provider,
            config,
            pause,
            rate_limiter,
            rx,
            shutdown,
            conn,
            syncing_count,
        }
    }

    /// Run the scheduler loop until shutdown.
    pub async fn run(mut self) {
        let mut pending: VecDeque<String> = VecDeque::new();
        let poll_interval = Duration::from_secs(self.config.sync.poll_interval_secs as u64);
        let mut poll_tick = tokio::time::Instant::now() + poll_interval;

        loop {
            if self.shutdown.load(Ordering::Relaxed) {
                info!("scheduler shutting down");
                break;
            }

            tokio::select! {
                Some(req) = self.rx.recv() => {
                    if !pending.contains(&req.sync_folder_id) {
                        pending.push_back(req.sync_folder_id);
                    }
                }
                _ = tokio::time::sleep_until(poll_tick) => {
                    poll_tick += poll_interval;
                    let folders = match list_all_folders(&self.pool).await {
                        Ok(f) => f,
                        Err(e) => {
                            error!(error = %e, "list sync folders failed");
                            continue;
                        }
                    };
                    for f in folders {
                        if !f.paused && !pending.contains(&f.id) {
                            pending.push_back(f.id);
                        }
                    }
                }
            }

            while let Some(folder_id) = pending.pop_front() {
                if self.shutdown.load(Ordering::Relaxed) {
                    break;
                }
                if !self.rate_limiter.try_acquire() {
                    pending.push_front(folder_id);
                    tokio::time::sleep(Duration::from_millis(200)).await;
                    continue;
                }
                let folder = match SyncFolderRepository::get_by_id(&self.pool, &folder_id).await {
                    Ok(Some(f)) => f,
                    Ok(None) => continue,
                    Err(e) => {
                        warn!(error = %e, "get sync folder failed");
                        continue;
                    }
                };
                if folder.paused {
                    continue;
                }
                match run_sync_loop(
                    &self.pool,
                    &self.drive_client,
                    Arc::clone(&self.token_provider),
                    &self.config,
                    &folder,
                    &self.pause,
                    self.conn.as_ref(),
                    self.syncing_count.as_ref(),
                )
                .await
                {
                    Ok(n) => {
                        debug!(folder_id = %folder_id, files_synced = n, "sync completed");
                    }
                    Err(e) => {
                        warn!(folder_id = %folder_id, error = %e, "sync failed");
                        let err_id = uuid::Uuid::new_v4().to_string();
                        let _ = SyncErrorRepository::insert(
                            &self.pool,
                            &err_id,
                            None,
                            &e.to_string(),
                            Utc::now(),
                            0,
                        )
                        .await;
                        pending.push_back(folder_id);
                    }
                }
            }
        }
    }
}

/// List all sync folders (convenience for scheduler/D-Bus). Not in gds-core; we use list_by_account and collect.
async fn list_all_folders(pool: &SqlitePool) -> Result<Vec<SyncFolder>, sqlx::Error> {
    let accounts = AccountRepository::list_all(pool).await?;
    let mut folders = Vec::new();
    for a in accounts {
        let f = SyncFolderRepository::list_by_account(pool, &a.id).await?;
        folders.extend(f);
    }
    Ok(folders)
}
