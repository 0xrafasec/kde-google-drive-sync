//! Google Drive Sync daemon — background service.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use gds_core::api::DriveClient;
use gds_core::auth::{KeyringTokenStore, TokenProvider};
use gds_core::db::{
    create_pool_from_path, get_oauth_app_credentials, run_migrations, SyncFolderRepository,
};
use gds_core::model::Config;
use gds_daemon::dbus::{DaemonService, DaemonState};
use gds_daemon::scheduler::{Scheduler, TokenBucket};
use gds_daemon::watcher::{FileWatcher, WatchEvent};
use tokio::sync::mpsc;
use tracing::{info, warn};
use zbus::connection;

const DAEMON_NAME: &str = "org.kde.GDriveSync";
const OBJECT_PATH: &str = "/org/kde/GDriveSync";

#[derive(Parser, Debug)]
#[command(about = "Google Drive Sync daemon")]
struct Args {
    /// Config directory (default: $GDS_CONFIG_DIR or ~/.config/gds)
    #[arg(long)]
    config_dir: Option<PathBuf>,

    /// Data directory (default: $GDS_DATA_DIR or ~/.local/share/gds)
    #[arg(long)]
    data_dir: Option<PathBuf>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, short)]
    log_level: Option<String>,

    /// Run in foreground (no PID file)
    #[arg(long, short)]
    foreground: bool,
}

fn config_dir() -> PathBuf {
    std::env::var("GDS_CONFIG_DIR")
        .ok()
        .map(PathBuf::from)
        .or_else(|| dirs::config_dir().map(|d| d.join("gds")))
        .unwrap_or_else(|| PathBuf::from(".").join(".config").join("gds"))
}

fn data_dir() -> PathBuf {
    std::env::var("GDS_DATA_DIR")
        .ok()
        .map(PathBuf::from)
        .or_else(|| dirs::data_local_dir().map(|d| d.join("gds")))
        .unwrap_or_else(|| PathBuf::from(".").join(".local").join("share").join("gds"))
}

fn load_config(config_dir: &std::path::Path) -> Result<Config> {
    let path = config_dir.join("config.toml");
    let s = std::fs::read_to_string(&path)
        .with_context(|| format!("read config {}", path.display()))?;
    toml::from_str(&s).with_context(|| format!("parse config {}", path.display()))
}

/// Resolve OAuth client_id and client_secret from the DB only.
async fn resolve_oauth_credentials(pool: &sqlx::SqlitePool) -> (String, Option<String>) {
    get_oauth_app_credentials(pool)
        .await
        .ok()
        .flatten()
        .map(|(id, secret)| (id, Some(secret)))
        .unwrap_or_else(|| (String::new(), None))
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let log_level = args
        .log_level
        .or_else(|| std::env::var("RUST_LOG").ok())
        .unwrap_or_else(|| "info".to_string());
    tracing_subscriber::fmt()
        .with_env_filter(log_level)
        .with_writer(std::io::stderr)
        .init();

    let config_dir = args.config_dir.unwrap_or_else(config_dir);
    let data_dir = args.data_dir.unwrap_or_else(data_dir);

    // Ensure config dir exists (RPM/DEB/Flatpak do not create ~/.config/gds).
    std::fs::create_dir_all(&config_dir)
        .with_context(|| format!("create config dir {}", config_dir.display()))?;

    let config = load_config(&config_dir).unwrap_or_else(|e| {
        warn!(error = %e, "load config failed, using defaults");
        Config::default()
    });

    std::fs::create_dir_all(&data_dir)
        .with_context(|| format!("create data dir {}", data_dir.display()))?;

    let db_path = data_dir.join("state.db");
    let pool = create_pool_from_path(&db_path)
        .await
        .context("create db pool")?;
    run_migrations(&pool).await.context("run migrations")?;

    let (resolved_client_id, resolved_client_secret) = resolve_oauth_credentials(&pool).await;

    if resolved_client_id.is_empty() || resolved_client_secret.is_none() {
        warn!(
            "No OAuth credentials in the database. Run 'gdrivesync configure' to set up Client ID and Client Secret, then restart the daemon."
        );
    }

    let store = Arc::new(KeyringTokenStore);
    let token_provider = Arc::new(
        TokenProvider::new(
            &resolved_client_id,
            resolved_client_secret.as_deref(),
            config.oauth.redirect_port,
            store.clone(),
        )
        .map_err(anyhow::Error::msg)?,
    );
    let drive_client = DriveClient::new(&config).map_err(anyhow::Error::msg)?;

    let pause = Arc::new(AtomicBool::new(false));
    let syncing_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let shutdown = Arc::new(AtomicBool::new(false));
    let (sync_tx, sync_rx) = mpsc::unbounded_channel();

    let state = Arc::new(DaemonState {
        pool: pool.clone(),
        config: config.clone(),
        resolved_client_id: resolved_client_id.clone(),
        resolved_client_secret: resolved_client_secret.clone(),
        token_provider: token_provider.clone(),
        token_store: store,
        drive_client: drive_client.clone(),
        pause: pause.clone(),
        syncing_count: syncing_count.clone(),
        sync_request_tx: sync_tx.clone(),
    });

    let conn = Arc::new(
        connection::Builder::session()
            .context("session bus")?
            .name(DAEMON_NAME)
            .context("request bus name (is another daemon running?)")?
            .serve_at(
                OBJECT_PATH,
                DaemonService {
                    state: state.clone(),
                },
            )
            .context("serve D-Bus object")?
            .build()
            .await
            .context("build connection")?,
    );

    info!("D-Bus service registered as {}", DAEMON_NAME);

    let rate_limiter = Arc::new(TokenBucket::new(2));
    let scheduler = Scheduler::new(
        pool.clone(),
        drive_client,
        token_provider,
        config.clone(),
        pause,
        rate_limiter,
        sync_rx,
        shutdown.clone(),
        conn.clone(),
        syncing_count.clone(),
    );
    let scheduler_handle = tokio::spawn(scheduler.run());

    let folders = list_all_folders_inner(&pool).await.unwrap_or_default();
    let debounce_ms = 500u64;
    for folder in folders {
        if folder.paused {
            continue;
        }
        let path = PathBuf::from(&folder.local_path);
        if path.exists() {
            let watcher = match FileWatcher::new(path, debounce_ms).start() {
                Ok(rx) => rx,
                Err(e) => {
                    warn!(path = %folder.local_path, error = %e, "watcher start failed");
                    continue;
                }
            };
            let sync_tx_w = sync_tx.clone();
            let folder_id = folder.id.clone();
            std::thread::spawn(move || {
                while let Ok(ev) = watcher.recv() {
                    if let WatchEvent::Changed(_) = ev {
                        let _ = sync_tx_w.send(gds_daemon::scheduler::SyncRequest {
                            sync_folder_id: folder_id.clone(),
                        });
                    }
                }
            });
        }
    }

    let pid_path = data_dir.join("daemon.pid");
    if !args.foreground {
        if let Err(e) = std::fs::write(&pid_path, std::process::id().to_string()) {
            warn!(error = %e, "write PID file failed");
        }
    }

    let shutdown_c = shutdown.clone();
    let sig_handle = tokio::spawn(async move {
        #[cfg(unix)]
        {
            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .unwrap_or_else(|e| panic!("signal: {}", e));
            let mut sigint =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
                    .unwrap_or_else(|e| panic!("signal: {}", e));
            tokio::select! {
                _ = sigterm.recv() => {}
                _ = sigint.recv() => {}
                _ = tokio::signal::ctrl_c() => {}
            }
        }
        #[cfg(not(unix))]
        {
            tokio::signal::ctrl_c().await.ok();
        }
        shutdown_c.store(true, Ordering::Relaxed);
    });

    tokio::select! {
        _ = sig_handle => {}
        _ = scheduler_handle => {}
    }

    if !args.foreground && pid_path.exists() {
        let _ = std::fs::remove_file(&pid_path);
    }
    info!("daemon shutting down");
    Ok(())
}

async fn list_all_folders_inner(
    pool: &sqlx::SqlitePool,
) -> Result<Vec<gds_core::model::SyncFolder>, sqlx::Error> {
    let accounts = gds_core::db::AccountRepository::list_all(pool).await?;
    let mut folders = Vec::new();
    for a in accounts {
        let f = SyncFolderRepository::list_by_account(pool, &a.id).await?;
        folders.extend(f);
    }
    Ok(folders)
}
