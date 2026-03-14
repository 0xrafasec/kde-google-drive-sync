//! Integration tests: D-Bus + CLI `run` (session bus).
//! Skipped without `DBUS_SESSION_BUS_ADDRESS`.

use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU32};
use std::sync::Arc;

use gds_cli::cli::{Cli, Command as CliCmd};
use gds_core::api::DriveClient;
use gds_core::auth::{InMemoryTokenStore, TokenProvider};
use gds_core::db::{create_pool, run_migrations};
use gds_core::model::Config;
use gds_daemon::dbus::{DaemonService, DaemonState};
use tokio::sync::mpsc;
use zbus::connection;

#[tokio::test]
async fn status_json_exits_zero_when_daemon_on_test_bus() {
    if std::env::var("DBUS_SESSION_BUS_ADDRESS").is_err() {
        eprintln!("skip: no session bus");
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
    let (tx, _rx) = mpsc::unbounded_channel();
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
    let name = format!("org.kde.GDriveSync.CliTest{}", unique);
    let path = format!("/org/kde/GDriveSync/CliTest{}", unique);

    let _conn = connection::Builder::session()
        .expect("session")
        .name(name.clone())
        .expect("name")
        .serve_at(path.clone(), service)
        .expect("serve_at")
        .build()
        .await
        .expect("build");

    std::env::set_var("GDS_DBUS_SERVICE", &name);
    std::env::set_var("GDS_DBUS_PATH", &path);
    let cli = Cli {
        json: true,
        quiet: false,
        verbose: false,
        command: CliCmd::Status,
    };
    let code = gds_cli::run::run(cli).await.expect("run");
    assert_eq!(code, 0);
    std::env::remove_var("GDS_DBUS_SERVICE");
    std::env::remove_var("GDS_DBUS_PATH");
}

#[tokio::test]
async fn daemon_unreachable_exits_two() {
    if std::env::var("DBUS_SESSION_BUS_ADDRESS").is_err() {
        return;
    }
    let exe = env!("CARGO_BIN_EXE_gdrivesync");
    let out = Command::new(exe)
        .args(["--json", "accounts", "list"])
        .env("GDS_DBUS_SERVICE", "org.kde.GDriveSync.NonExistent999")
        .env("GDS_DBUS_PATH", "/org/kde/GDriveSync/DoesNotExist")
        .output()
        .expect("spawn");
    assert_eq!(
        out.status.code(),
        Some(2),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn completions_bash_nonzero_stdout() {
    let exe = env!("CARGO_BIN_EXE_gdrivesync");
    let out = Command::new(exe)
        .args(["completions", "bash"])
        .output()
        .expect("spawn");
    assert!(out.status.success());
    assert!(!out.stdout.is_empty());
}

#[test]
fn daemon_status_json_runs_without_bus() {
    let exe = env!("CARGO_BIN_EXE_gdrivesync");
    let out = Command::new(exe)
        .args(["--json", "daemon", "status"])
        .output()
        .expect("spawn");
    assert!(out.status.success());
    let j: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(j.get("on_bus").is_some());
}
