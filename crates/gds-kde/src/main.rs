//! KDE UI: system tray (`ksni`) + freedesktop notifications.

mod dbus;
mod notifications;
mod prefs;
mod state;
mod tray;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use futures_util::StreamExt;
use ksni::TrayMethods;
use tokio::sync::{mpsc, Mutex};
use zbus::message::Type as MsgType;
use zbus::MessageStream;

use crate::dbus::{connect_session, daemon_proxy, BusTarget};
use crate::notifications::NotificationManager;
use crate::prefs::load_config;
use crate::state::{SharedUiState, UiState};
use crate::tray::{GDriveTray, TrayAction};

fn data_dir() -> PathBuf {
    std::env::var("GDS_DATA_DIR")
        .ok()
        .map(PathBuf::from)
        .or_else(|| dirs::data_local_dir().map(|d| d.join("gds")))
        .unwrap_or_else(|| PathBuf::from(".local/share/gds"))
}

fn icon_dir() -> PathBuf {
    std::env::var("GDS_ICON_PATH")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("assets")
                .join("icons")
        })
}

#[tokio::main(flavor = "multi_thread", worker_threads = 4)]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()))
        .init();

    let cfg = load_config();
    let conn = connect_session().await?;
    let target = BusTarget::default();
    let ui: SharedUiState = Arc::new(Mutex::new(UiState::new()));
    let (tx, mut rx) = mpsc::unbounded_channel::<TrayAction>();

    let data_d = data_dir();
    let _ = std::fs::create_dir_all(&data_d);
    let nm = Arc::new(Mutex::new(NotificationManager::new(
        conn.clone(),
        cfg.ui.show_notifications,
        cfg.ui.notification_timeout_ms,
        &data_d,
    )));

    let tray_ui = Arc::clone(&ui);
    let icon_path = icon_dir();
    let tray = GDriveTray::new(tray_ui, icon_path.clone(), tx);
    let handle = tray.spawn().await.map_err(|e| anyhow::anyhow!("{}", e))?;

    let conn_refresh = conn.clone();
    let target_refresh = target.clone();
    let ui_refresh = Arc::clone(&ui);
    let handle_refresh = handle.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(3));
        loop {
            tick.tick().await;
            match daemon_proxy(&conn_refresh, &target_refresh).await {
                Ok(proxy) => match proxy.get_status().await {
                    Ok((s, n)) => {
                        let mut g = ui_refresh.lock().await;
                        g.connected = true;
                        g.status_str = s;
                        g.syncing_count = n;
                        if let Ok(accs) = proxy.get_accounts().await {
                            g.accounts = accs;
                        }
                        if let Ok(folders) = proxy.get_sync_folders().await {
                            g.folders = folders;
                        }
                        drop(g);
                        let _ = handle_refresh.update(|_: &mut GDriveTray| {}).await;
                    }
                    Err(_) => {
                        let mut g = ui_refresh.lock().await;
                        g.connected = false;
                        drop(g);
                        let _ = handle_refresh.update(|_: &mut GDriveTray| {}).await;
                    }
                },
                Err(_) => {
                    let mut g = ui_refresh.lock().await;
                    g.connected = false;
                    g.status_str = "offline".to_string();
                    drop(g);
                    let _ = handle_refresh.update(|_: &mut GDriveTray| {}).await;
                }
            }
        }
    });

    let conn_sig = conn.clone();
    let nm_sig = Arc::clone(&nm);
    let ui_sig = Arc::clone(&ui);
    let handle_sig = handle.clone();
    tokio::spawn(async move {
        let mut stream = MessageStream::from(&conn_sig);
        while let Some(msg) = stream.next().await {
            let Ok(msg) = msg else { continue };
            if msg.message_type() != MsgType::Signal {
                continue;
            }
            let h = msg.header();
            let iface = h.interface().map(|i| i.as_str()).unwrap_or("");
            if iface != "org.kde.GDriveSync.Daemon" {
                continue;
            }
            let member = h.member().map(|m| m.as_str()).unwrap_or("");
            match member {
                "SyncCompleted" => {
                    if let Ok((_, path, files)) =
                        msg.body().deserialize::<(String, String, u32)>()
                    {
                        ui_sig.lock().await.push_activity(format!(
                            "Synced {} files — {}",
                            files, path
                        ));
                        let n = nm_sig.lock().await;
                        let _ = n.sync_complete(&path, files).await;
                    }
                }
                "ConflictDetected" => {
                    if let Ok((local_path, conflict_copy)) =
                        msg.body().deserialize::<(String, String)>()
                    {
                        ui_sig.lock().await.push_activity(format!(
                            "Conflict: {}",
                            local_path
                        ));
                        let n = nm_sig.lock().await;
                        let _ = n.conflict(&local_path, &conflict_copy).await;
                    }
                }
                "SyncError" => {
                    if let Ok((_, path, err)) =
                        msg.body().deserialize::<(String, String, String)>()
                    {
                        ui_sig.lock().await.push_activity(format!(
                            "Error {}: {}",
                            path, err
                        ));
                        let mut n = nm_sig.lock().await; // sync_error dedup mutates
                        let _ = n.sync_error(&err, &path).await;
                    }
                }
                "SyncStarted" => {
                    if let Ok((_, path)) = msg.body().deserialize::<(String, String)>() {
                        let mut u = ui_sig.lock().await;
                        u.push_activity(format!("Sync started: {}", path));
                        drop(u);
                        let mut n = nm_sig.lock().await; // initial_sync_started mutates marker
                        let _ = n.initial_sync_started().await;
                    }
                }
                "StatusChanged" => {
                    let _ = handle_sig.update(|_: &mut GDriveTray| {}).await;
                }
                _ => {}
            }
        }
    });

    let conn_quota = conn.clone();
    let target_quota = target.clone();
    let nm_quota = Arc::clone(&nm);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(600));
        loop {
            interval.tick().await;
            let proxy = match daemon_proxy(&conn_quota, &target_quota).await {
                Ok(p) => p,
                Err(_) => continue,
            };
            let Ok(accs) = proxy.get_accounts().await else { continue };
            for a in accs {
                let Ok((lim_s, use_s, _)) = proxy.get_about_info(&a.id).await else {
                    continue;
                };
                let lim: i64 = lim_s.parse().unwrap_or(0);
                let usage: i64 = use_s.parse().unwrap_or(0);
                if lim > 0 {
                    let free = (lim - usage) as f64 / lim as f64;
                    if free < 0.1 && free >= 0.0 {
                        let n = nm_quota.lock().await;
                        let _ = n.low_quota(&a.email, free).await;
                    }
                }
            }
        }
    });

    let conn_actions = conn.clone();
    let target_actions = target.clone();
    let ui_log = Arc::clone(&ui);
    while let Some(action) = rx.recv().await {
        match action {
            TrayAction::Quit => std::process::exit(0),
            TrayAction::OpenBrowser => {
                let _ = tokio::process::Command::new("xdg-open")
                    .arg("https://drive.google.com")
                    .spawn();
            }
            TrayAction::OpenFolder(path) => {
                let _ = tokio::process::Command::new("xdg-open").arg(&path).spawn();
            }
            TrayAction::Pause => {
                if let Ok(p) = daemon_proxy(&conn_actions, &target_actions).await {
                    let _ = p.pause_sync().await;
                }
            }
            TrayAction::Resume => {
                if let Ok(p) = daemon_proxy(&conn_actions, &target_actions).await {
                    let _ = p.resume_sync().await;
                }
            }
            TrayAction::ForceSync(path) => {
                if let Ok(p) = daemon_proxy(&conn_actions, &target_actions).await {
                    let _ = p.force_sync(&path).await;
                }
            }
            TrayAction::Preferences => {
                let c = load_config();
                let _ = prefs::open_preferences_dialog(&c);
                let _ = handle.update(|_: &mut GDriveTray| {}).await;
            }
            TrayAction::ActivityLog => {
                let g = ui_log.lock().await;
                let mut text = String::new();
                for e in g.activity.iter().rev().take(200) {
                    text.push_str(&format!(
                        "[{}] {}\n",
                        e.at.format("%Y-%m-%d %H:%M:%S"),
                        e.message
                    ));
                }
                drop(g);
                let tmp = data_d.join("gds_activity_log.txt");
                if std::fs::write(&tmp, &text).is_ok() {
                    let _ = tokio::process::Command::new("kdialog")
                        .args([
                            "--textbox",
                            tmp.to_str().unwrap_or(""),
                            "600",
                            "400",
                        ])
                        .spawn();
                }
            }
        }
    }

    Ok(())
}
