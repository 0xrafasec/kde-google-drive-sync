//! org.freedesktop.Notifications — sync events, conflicts, quota.

use std::collections::HashMap;

use anyhow::{Context, Result};
use tracing::warn;
use zbus::zvariant::Value;

const APP: &str = "Google Drive Sync";

const ERROR_DEDUP_SECS: u64 = 300;

pub struct NotificationManager {
    conn: zbus::Connection,
    show: bool,
    timeout_ms: i32,
    last_errors: HashMap<String, std::time::Instant>,
    first_run_path: std::path::PathBuf,
}

impl NotificationManager {
    pub fn new(conn: zbus::Connection, show: bool, timeout_ms: u32, data_dir: &std::path::Path) -> Self {
        Self {
            conn,
            show,
            timeout_ms: (timeout_ms as i32).min(i32::MAX),
            last_errors: HashMap::new(),
            first_run_path: data_dir.join("kde_first_sync_notified"),
        }
    }

    async fn proxy(&self) -> zbus::Result<zbus::Proxy<'_>> {
        zbus::Proxy::new(
            &self.conn,
            "org.freedesktop.Notifications",
            "/org/freedesktop/Notifications",
            "org.freedesktop.Notifications",
        )
        .await
    }

    async fn notify(
        &self,
        icon: &str,
        summary: &str,
        body: &str,
        actions: Vec<String>,
        timeout: i32,
    ) -> Result<u32> {
        if !self.show {
            return Ok(0);
        }
        let p = self.proxy().await.context("Notifications")?;
        let hints: HashMap<&str, Value<'_>> = [(
            "desktop-entry",
            Value::new("org.kde.gdrivesync".to_string()),
        )]
        .into_iter()
        .collect();
        let id: u32 = p
            .call(
                "Notify",
                &(
                    APP,
                    0u32,
                    icon,
                    summary,
                    body,
                    actions,
                    &hints,
                    timeout,
                ),
            )
            .await
            .context("Notify")?;
        Ok(id)
    }

    pub async fn sync_complete(&self, path: &str, files: u32) -> Result<()> {
        let _ = self
            .notify(
                "folder-sync",
                "Sync complete",
                &format!("{} — {} file(s) synced.", path, files),
                vec![],
                self.timeout_ms,
            )
            .await?;
        Ok(())
    }

    pub async fn conflict(&self, _local_path: &str, conflict_copy: &str) -> Result<u32> {
        self.notify(
            "dialog-warning",
            "Sync conflict",
            &format!(
                "Server copy kept. Your version saved as: {}",
                conflict_copy
            ),
            vec![
                "keep".into(),
                "Keep Mine".into(),
                "diff".into(),
                "View Diff".into(),
                "dismiss".into(),
                "Dismiss".into(),
            ],
            -1,
        )
        .await
    }

    pub async fn sync_error(&mut self, err: &str, path: &str) -> Result<()> {
        let key = format!("{}:{}", path, err);
        if let Some(t) = self.last_errors.get(&key) {
            if t.elapsed().as_secs() < ERROR_DEDUP_SECS {
                return Ok(());
            }
        }
        self.last_errors.insert(key, std::time::Instant::now());
        let _ = self
            .notify(
                "dialog-error",
                "Sync error",
                &format!("{}\n{}", path, err),
                vec!["retry".into(), "Retry".into()],
                -1,
            )
            .await?;
        Ok(())
    }

    pub async fn low_quota(&self, account: &str, pct_free: f64) -> Result<()> {
        let _ = self
            .notify(
                "drive-harddisk",
                "Low Google Drive space",
                &format!("{} — {:.0}% free.", account, pct_free * 100.0),
                vec![],
                self.timeout_ms,
            )
            .await?;
        Ok(())
    }

    pub async fn initial_sync_started(&mut self) -> Result<()> {
        if self.first_run_path.exists() {
            return Ok(());
        }
        let _ = self
            .notify(
                "folder-sync",
                "Initial sync started",
                "First-time sync is running in the background.",
                vec![],
                self.timeout_ms,
            )
            .await?;
        if let Err(e) = std::fs::write(&self.first_run_path, b"1") {
            warn!(error = %e, "kde first-run marker");
        }
        Ok(())
    }
}

/// Launch diff tool: KDiff3, meld, or terminal `diff`. Used when user invokes “View Diff” from notification (wired via Freedesktop action in a full session).
#[allow(dead_code)]
pub async fn launch_diff_tool(a: &str, b: &str) {
    for cmd in ["kdiff3", "meld"] {
        if tokio::process::Command::new(cmd)
            .args([a, b])
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false)
        {
            return;
        }
    }
    let _ = tokio::process::Command::new("konsole")
        .args(["-e", "diff", "-u", a, b])
        .spawn();
}
