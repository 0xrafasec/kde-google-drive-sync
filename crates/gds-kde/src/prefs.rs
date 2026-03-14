//! Preferences: edit `~/.config/gds/config.toml` (sync interval, UI notify).

use std::path::Path;

use anyhow::{Context, Result};
use gds_core::model::Config;

pub fn config_path() -> std::path::PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| Path::new(".").join(".config"))
        .join("gds")
        .join("config.toml")
}

pub fn load_config() -> Config {
    let p = config_path();
    std::fs::read_to_string(&p)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_config(c: &Config) -> Result<()> {
    let p = config_path();
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d).with_context(|| format!("mkdir {}", d.display()))?;
    }
    let s =
        toml::to_string_pretty(c).context("serialize config")?;
    std::fs::write(&p, s).with_context(|| format!("write {}", p.display()))?;
    Ok(())
}

/// Opens kdialog (preferred) or zenity to edit poll interval and notification toggle.
pub fn open_preferences_dialog(current: &Config) -> Result<Option<Config>> {
    let poll = current.sync.poll_interval_secs;
    let notify = current.ui.show_notifications;
    let conflict = "server_wins";

    // kdialog: --title + forms
    let out = std::process::Command::new("kdialog")
        .args([
            "--title",
            "Google Drive Sync — Preferences",
            "--inputbox",
            &format!(
                "Poll interval (seconds) [current: {}]\n(also edit {} for full options)",
                poll,
                config_path().display()
            ),
            &poll.to_string(),
        ])
        .output();

    match out {
        Ok(o) if o.status.success() => {
            let line = String::from_utf8_lossy(&o.stdout);
            let line = line.trim();
            let poll_new: u32 = line.parse().unwrap_or(poll).max(5).min(3600);
            let mut c = current.clone();
            c.sync.poll_interval_secs = poll_new;
            let notify_out = std::process::Command::new("kdialog")
                .args([
                    "--title",
                    "Notifications",
                    "--yesno",
                    "Show desktop notifications?",
                ])
                .status();
            if let Ok(s) = notify_out {
                c.ui.show_notifications = s.success();
            } else {
                c.ui.show_notifications = notify;
            }
            let _ = conflict;
            save_config(&c)?;
            Ok(Some(c))
        }
        _ => {
            // zenity fallback
            let z = std::process::Command::new("zenity")
                .args([
                    "--entry",
                    "--title=GDrive Sync",
                    &format!("Poll interval (s), current {}", poll),
                    &format!("--text={}", poll),
                ])
                .output();
            match z {
                Ok(o) if o.status.success() => {
                    let line = String::from_utf8_lossy(&o.stdout);
                    let poll_new: u32 = line.trim().parse().unwrap_or(poll).max(5);
                    let mut c = current.clone();
                    c.sync.poll_interval_secs = poll_new;
                    save_config(&c)?;
                    Ok(Some(c))
                }
                _ => Ok(None),
            }
        }
    }
}
