//! Integration test for D-Bus service: register and call GetStatus.
//! Requires a session bus (e.g. DBUS_SESSION_BUS_ADDRESS); skipped in CI without it.

use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::Arc;

use gds_core::api::DriveClient;
use gds_core::auth::InMemoryTokenStore;
use gds_core::auth::TokenProvider;
use gds_core::db::{create_pool, run_migrations};
use gds_core::model::Config;
use gds_daemon::dbus::{DaemonService, DaemonState};
use tokio::sync::mpsc;
use zbus::connection;
use zbus::proxy;

#[proxy(
    interface = "org.kde.GDriveSync.Daemon",
    default_service = "org.kde.GDriveSync",
    default_path = "/org/kde/GDriveSync"
)]
trait Daemon {
    async fn get_status(&self) -> zbus::Result<(String, u32)>;
}

#[tokio::test]
async fn dbus_get_status_returns_idle_when_no_sync() {
    if std::env::var("DBUS_SESSION_BUS_ADDRESS").is_err() {
        eprintln!("Skipping D-Bus test: no session bus");
        return;
    }

    let pool = create_pool("sqlite::memory:").await.expect("pool");
    run_migrations(&pool).await.expect("migrations");
    let config = Config::default();
    let store = Arc::new(InMemoryTokenStore::new());
    let token_provider = Arc::new(
        TokenProvider::new(
            &config.oauth.client_id,
            None,
            config.oauth.redirect_port,
            store.clone(),
        )
        .expect("token provider"),
    );
    let drive_client = DriveClient::new(&config).expect("drive client");
    let (tx, rx) = mpsc::unbounded_channel();
    let state = Arc::new(DaemonState {
        pool,
        config: config.clone(),
        resolved_client_id: config.oauth.client_id.clone(),
        resolved_client_secret: None,
        token_provider,
        token_store: store,
        drive_client,
        pause: Arc::new(AtomicBool::new(false)),
        syncing_count: Arc::new(AtomicU32::new(0)),
        sync_request_tx: tx,
    });
    let service = DaemonService { state };

    let unique = std::process::id();
    let name = format!("org.kde.GDriveSync.Test{}", unique);
    let path = format!("/org/kde/GDriveSync/Test{}", unique);

    let conn = connection::Builder::session()
        .expect("session")
        .name(name.clone())
        .expect("name")
        .serve_at(path.clone(), service)
        .expect("serve_at")
        .build()
        .await
        .expect("build");

    let proxy: DaemonProxy<'_> = DaemonProxy::builder(&conn)
        .path(path.as_str())
        .expect("path")
        .destination(name.as_str())
        .expect("destination")
        .build()
        .await
        .expect("proxy");

    let (status, count) = proxy.get_status().await.expect("get_status");
    assert_eq!(status, "idle");
    assert_eq!(count, 0);

    drop(rx);
}
