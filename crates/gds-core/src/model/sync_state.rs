//! Sync state for a file (per-file status).

use serde::{Deserialize, Serialize};

/// Per-file sync state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncStateKind {
    Synced,
    Pending,
    Conflict,
    Error,
    Uploading,
    Downloading,
}

/// Sync state with optional message (e.g. for Error).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncState {
    pub kind: SyncStateKind,
    #[serde(default)]
    pub message: Option<String>,
}

impl SyncState {
    pub const fn synced() -> Self {
        Self {
            kind: SyncStateKind::Synced,
            message: None,
        }
    }

    pub const fn pending() -> Self {
        Self {
            kind: SyncStateKind::Pending,
            message: None,
        }
    }

    pub const fn conflict() -> Self {
        Self {
            kind: SyncStateKind::Conflict,
            message: None,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            kind: SyncStateKind::Error,
            message: Some(msg.into()),
        }
    }

    pub const fn uploading() -> Self {
        Self {
            kind: SyncStateKind::Uploading,
            message: None,
        }
    }

    pub const fn downloading() -> Self {
        Self {
            kind: SyncStateKind::Downloading,
            message: None,
        }
    }
}

impl Default for SyncState {
    fn default() -> Self {
        Self::pending()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_state_constructors() {
        assert_eq!(SyncState::synced().kind, SyncStateKind::Synced);
        assert_eq!(SyncState::pending().kind, SyncStateKind::Pending);
        assert_eq!(SyncState::conflict().kind, SyncStateKind::Conflict);
        assert_eq!(SyncState::error("msg").message.as_deref(), Some("msg"));
        assert_eq!(SyncState::uploading().kind, SyncStateKind::Uploading);
        assert_eq!(SyncState::downloading().kind, SyncStateKind::Downloading);
    }

    #[test]
    fn sync_state_default_is_pending() {
        assert_eq!(SyncState::default().kind, SyncStateKind::Pending);
    }

    #[test]
    fn sync_state_serialization_roundtrip() {
        let states = [
            SyncState::synced(),
            SyncState::pending(),
            SyncState::error("err".to_string()),
        ];
        for s in &states {
            let json = serde_json::to_string(s).unwrap();
            let s2: SyncState = serde_json::from_str(&json).unwrap();
            assert_eq!(s.kind, s2.kind);
            assert_eq!(s.message, s2.message);
        }
    }
}
