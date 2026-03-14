//! Start/stop daemon outside D-Bus (PID file + optional systemd).

use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

use tokio::time::sleep;

use crate::dbus_client::{BusTarget, DEFAULT_SERVICE};

/// Same semantics as `gds-daemon` main.
pub fn data_dir() -> PathBuf {
    std::env::var("GDS_DATA_DIR")
        .ok()
        .map(PathBuf::from)
        .or_else(|| dirs::data_local_dir().map(|d| d.join("gds")))
        .unwrap_or_else(|| PathBuf::from(".local/share/gds"))
}

pub fn pid_path() -> PathBuf {
    data_dir().join("daemon.pid")
}

/// Try systemd user unit first; if unit missing or command fails, spawn `gds-daemon`.
pub async fn daemon_start(verbose: bool) -> anyhow::Result<()> {
    let systemctl = Command::new("systemctl")
        .args(["--user", "start", "gds-daemon"])
        .status();
    match systemctl {
        Ok(s) if s.success() => {
            if verbose {
                eprintln!("systemctl --user start gds-daemon: ok");
            }
            wait_for_bus(Duration::from_secs(15), verbose).await?;
            return Ok(());
        }
        Ok(_) | Err(_) => {}
    }
    let bin = which_gds_daemon();
    let mut cmd = Command::new(&bin);
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::null());
    cmd.stderr(std::process::Stdio::null());
    let child = cmd.spawn().map_err(|e| {
        anyhow::anyhow!("failed to spawn gds-daemon ({bin}): {e}. Is gds-daemon on PATH?")
    })?;
    drop(child);
    wait_for_bus(Duration::from_secs(20), verbose).await?;
    Ok(())
}

fn which_gds_daemon() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("gds-daemon")))
        .filter(|p| p.exists())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "gds-daemon".to_string())
}

async fn wait_for_bus(timeout: Duration, verbose: bool) -> anyhow::Result<()> {
    let step = Duration::from_millis(200);
    let mut elapsed = Duration::ZERO;
    while elapsed < timeout {
        if bus_name_has_owner(DEFAULT_SERVICE).await {
            if verbose {
                eprintln!("daemon registered on D-Bus");
            }
            return Ok(());
        }
        sleep(step).await;
        elapsed += step;
    }
    anyhow::bail!("daemon did not appear on D-Bus within {:?}", timeout);
}

/// True if `org.kde.GDriveSync` owns a name on the session bus (daemon is up).
pub async fn daemon_listening() -> bool {
    bus_name_has_owner(DEFAULT_SERVICE).await
}

async fn bus_name_has_owner(name: &str) -> bool {
    let Ok(conn) = zbus::Connection::session().await else {
        return false;
    };
    let Ok(proxy) = zbus::fdo::DBusProxy::builder(&conn).build().await else {
        return false;
    };
    let name = match zbus::names::BusName::try_from(name) {
        Ok(n) => n,
        Err(_) => return false,
    };
    proxy.name_has_owner(name.as_ref()).await.unwrap_or(false)
}

pub fn daemon_stop() -> anyhow::Result<()> {
    let systemctl = Command::new("systemctl")
        .args(["--user", "stop", "gds-daemon"])
        .status();
    if let Ok(s) = &systemctl {
        if s.success() {
            return Ok(());
        }
    }
    let path = pid_path();
    let pid: i32 = std::fs::read_to_string(&path)
        .map_err(|_| anyhow::anyhow!("no PID file at {}; is daemon running?", path.display()))?
        .trim()
        .parse()
        .map_err(|_| anyhow::anyhow!("invalid PID in {}", path.display()))?;
    #[cfg(unix)]
    {
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGTERM);
        }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        anyhow::bail!("daemon stop on non-Unix not supported");
    }
    Ok(())
}

#[derive(serde::Serialize)]
pub struct DaemonStatusJson {
    pub on_bus: bool,
    pub service: String,
    pub pid_file: Option<String>,
    pub pid: Option<u32>,
}

pub async fn daemon_status_json(target: &BusTarget) -> DaemonStatusJson {
    let on_bus = bus_name_has_owner(&target.service).await;
    let path = pid_path();
    let (pid_file, pid) = if path.exists() {
        let s = path.display().to_string();
        let pid = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| t.trim().parse().ok());
        (Some(s), pid)
    } else {
        (None, None)
    };
    DaemonStatusJson {
        on_bus,
        service: target.service.clone(),
        pid_file,
        pid,
    }
}
