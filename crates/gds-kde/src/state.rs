//! Shared UI state (tray + notifications).

use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::dbus::{AccountInfo, SyncFolderInfo};

/// High-level tray status derived from daemon + connection.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayStatusKind {
    Disconnected,
    Paused,
    Syncing { count: u32 },
    Idle,
}

#[derive(Clone, Debug)]
pub struct ActivityEntry {
    pub at: chrono::DateTime<chrono::Utc>,
    pub message: String,
}

const ACTIVITY_MAX: usize = 500;

#[derive(Debug)]
pub struct UiState {
    pub connected: bool,
    pub status_str: String,
    pub syncing_count: u32,
    pub accounts: Vec<AccountInfo>,
    pub folders: Vec<SyncFolderInfo>,
    pub activity: VecDeque<ActivityEntry>,
}

impl UiState {
    pub fn new() -> Self {
        Self {
            connected: false,
            status_str: "offline".to_string(),
            syncing_count: 0,
            accounts: Vec::new(),
            folders: Vec::new(),
            activity: VecDeque::with_capacity(ACTIVITY_MAX + 1),
        }
    }

    pub fn push_activity(&mut self, message: String) {
        self.activity.push_back(ActivityEntry {
            at: chrono::Utc::now(),
            message,
        });
        while self.activity.len() > ACTIVITY_MAX {
            self.activity.pop_front();
        }
    }

    pub fn tray_kind(&self) -> TrayStatusKind {
        if !self.connected {
            return TrayStatusKind::Disconnected;
        }
        if self.status_str == "paused" {
            return TrayStatusKind::Paused;
        }
        if self.status_str == "syncing" || self.syncing_count > 0 {
            return TrayStatusKind::Syncing {
                count: self.syncing_count.max(1),
            };
        }
        TrayStatusKind::Idle
    }
}

impl Default for UiState {
    fn default() -> Self {
        Self::new()
    }
}

pub type SharedUiState = Arc<Mutex<UiState>>;
