//! StatusNotifierItem tray (`ksni`).

use std::path::PathBuf;

use ksni::menu::{StandardItem, SubMenu};
use ksni::{Category, MenuItem, Status, ToolTip, Tray};

use crate::state::{SharedUiState, TrayStatusKind};

/// Icon theme base name (hicolor ships `org.kde.gdrivesync` under assets).
pub const ICON_APP: &str = "org.kde.gdrivesync";

pub struct GDriveTray {
    pub state: SharedUiState,
    pub icon_base: PathBuf,
    tx: tokio::sync::mpsc::UnboundedSender<TrayAction>,
}

#[derive(Debug)]
pub enum TrayAction {
    OpenFolder(String),
    OpenBrowser,
    Pause,
    Resume,
    ForceSync(String),
    Preferences,
    ActivityLog,
    Quit,
}

impl GDriveTray {
    pub fn new(state: SharedUiState, icon_base: PathBuf, tx: tokio::sync::mpsc::UnboundedSender<TrayAction>) -> Self {
        Self {
            state,
            icon_base,
            tx,
        }
    }

}

impl Tray for GDriveTray {
    fn id(&self) -> String {
        "org.kde.gdrivesync.tray".into()
    }

    fn category(&self) -> Category {
        Category::ApplicationStatus
    }

    fn title(&self) -> String {
        "Google Drive Sync".into()
    }

    fn icon_name(&self) -> String {
        // Blocked on async state — sync read would need try_lock; menu uses cached icon name.
        ICON_APP.into()
    }

    fn icon_theme_path(&self) -> String {
        self.icon_base.to_string_lossy().into_owned()
    }

    fn status(&self) -> Status {
        // ksni updates when we call handle.update()
        Status::Active
    }

    fn tool_tip(&self) -> ToolTip {
        if let Ok(g) = self.state.try_lock() {
            let title = "Google Drive Sync".to_string();
            let subtitle = match g.tray_kind() {
                TrayStatusKind::Disconnected => "Daemon offline — start gds-daemon".to_string(),
                TrayStatusKind::Paused => "Paused".to_string(),
                TrayStatusKind::Syncing { count } => format!("Syncing (~{} ops)…", count),
                TrayStatusKind::Idle => {
                    let last = g
                        .folders
                        .iter()
                        .filter(|f| f.last_sync_at > 0)
                        .map(|f| f.last_sync_at)
                        .max()
                        .unwrap_or(0);
                    if last > 0 {
                        let t = chrono::DateTime::from_timestamp(last, 0)
                            .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                            .unwrap_or_default();
                        format!("Up to date · last sync {}", t)
                    } else {
                        "Up to date".to_string()
                    }
                }
            };
            let email = g
                .accounts
                .first()
                .map(|a| a.email.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "No account".into());
            ToolTip {
                icon_name: ICON_APP.into(),
                icon_pixmap: vec![],
                title,
                description: format!("{}\n{}", email, subtitle),
            }
        } else {
            ToolTip {
                icon_name: ICON_APP.into(),
                icon_pixmap: vec![],
                title: "Google Drive Sync".into(),
                description: String::new(),
            }
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let g = self.state.blocking_lock();
        let mut items: Vec<MenuItem<Self>> = Vec::new();

        for acc in &g.accounts {
            let email = if acc.email.is_empty() {
                acc.id.clone()
            } else {
                acc.email.clone()
            };
            let mut sub: Vec<MenuItem<Self>> = Vec::new();
            for f in g.folders.iter().filter(|x| x.account_id == acc.id) {
                let path = f.local_path.clone();
                sub.push(
                    StandardItem {
                        label: format!("Open {}", truncate_path(&path)),
                        icon_name: "folder-open".into(),
                        activate: {
                            let tx = self.tx.clone();
                            Box::new(move |_: &mut Self| {
                                let _ = tx.send(TrayAction::OpenFolder(path.clone()));
                            })
                        },
                        ..Default::default()
                    }
                    .into(),
                );
            }
            if sub.is_empty() {
                sub.push(
                    StandardItem {
                        label: "(no sync folders)".into(),
                        enabled: false,
                        ..Default::default()
                    }
                    .into(),
                );
            }
            items.push(
                SubMenu {
                    label: format!("● {}", email),
                    submenu: sub,
                    ..Default::default()
                }
                .into(),
            );
        }

        if g.accounts.is_empty() {
            items.push(
                StandardItem {
                    label: "(no accounts — use gdrivesync accounts add)".into(),
                    enabled: false,
                    ..Default::default()
                }
                .into(),
            );
        }

        items.push(MenuItem::Separator);
        items.push(
            StandardItem {
                label: "Open in Browser".into(),
                icon_name: "internet-web-browser".into(),
                activate: {
                    let tx = self.tx.clone();
                    Box::new(move |_: &mut Self| {
                        let _ = tx.send(TrayAction::OpenBrowser);
                    })
                },
                ..Default::default()
            }
            .into(),
        );

        let paused = g.status_str == "paused";
        items.push(
            StandardItem {
                label: if paused {
                    "Resume Syncing"
                } else {
                    "Pause Syncing"
                }
                .into(),
                icon_name: if paused {
                    "media-playback-start"
                } else {
                    "media-playback-pause"
                }
                .into(),
                activate: {
                    let tx = self.tx.clone();
                    Box::new(move |_: &mut Self| {
                        let _ = tx.send(if paused {
                            TrayAction::Resume
                        } else {
                            TrayAction::Pause
                        });
                    })
                },
                ..Default::default()
            }
            .into(),
        );

        items.push(
            StandardItem {
                label: "Force Sync Now".into(),
                icon_name: "view-refresh".into(),
                activate: {
                    let tx = self.tx.clone();
                    Box::new(move |_: &mut Self| {
                        let _ = tx.send(TrayAction::ForceSync(String::new()));
                    })
                },
                ..Default::default()
            }
            .into(),
        );

        items.push(MenuItem::Separator);
        items.push(
            StandardItem {
                label: "Preferences…".into(),
                icon_name: "preferences-system".into(),
                activate: {
                    let tx = self.tx.clone();
                    Box::new(move |_: &mut Self| {
                        let _ = tx.send(TrayAction::Preferences);
                    })
                },
                ..Default::default()
            }
            .into(),
        );
        items.push(
            StandardItem {
                label: "View Sync Activity…".into(),
                icon_name: "view-list-details".into(),
                activate: {
                    let tx = self.tx.clone();
                    Box::new(move |_: &mut Self| {
                        let _ = tx.send(TrayAction::ActivityLog);
                    })
                },
                ..Default::default()
            }
            .into(),
        );
        items.push(MenuItem::Separator);
        items.push(
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: {
                    let tx = self.tx.clone();
                    Box::new(move |_: &mut Self| {
                        let _ = tx.send(TrayAction::Quit);
                    })
                },
                ..Default::default()
            }
            .into(),
        );

        items
    }
}

fn truncate_path(p: &str) -> String {
    let pb = PathBuf::from(p);
    pb.file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.to_string())
        .unwrap_or_else(|| p.chars().take(40).collect())
}
