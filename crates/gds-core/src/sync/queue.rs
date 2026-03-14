//! Priority queue for sync operations (downloads before uploads for initial sync).

use std::collections::VecDeque;

use crate::sync::change::SyncAction;

/// Queue of sync actions. Order: downloads first, then conflicts, then uploads, then deletes.
pub struct SyncQueue {
    inner: VecDeque<SyncAction>,
}

impl SyncQueue {
    pub fn new() -> Self {
        Self {
            inner: VecDeque::new(),
        }
    }

    /// Builds a queue from a list of actions, sorted by priority (downloads before uploads).
    pub fn from_actions(mut actions: Vec<SyncAction>) -> Self {
        actions.sort_by_key(|a| (a.priority(), a.relative_path.clone()));
        Self {
            inner: actions.into_iter().collect(),
        }
    }

    pub fn push(&mut self, action: SyncAction) {
        let p = action.priority();
        let mut i = 0;
        for (j, a) in self.inner.iter().enumerate() {
            if a.priority() > p {
                i = j;
                break;
            }
            i = j + 1;
        }
        self.inner.insert(i, action);
    }

    pub fn pop(&mut self) -> Option<SyncAction> {
        self.inner.pop_front()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

impl Default for SyncQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::change::{SyncAction, SyncActionKind};

    #[test]
    fn queue_orders_downloads_before_uploads() {
        let mut q = SyncQueue::new();
        q.push(SyncAction::new_upload(
            "a".into(),
            None,
            "m1".into(),
            chrono::Utc::now(),
        ));
        q.push(SyncAction::new_download(
            "b".into(),
            crate::model::DriveFile {
                id: "id".into(),
                name: "b".into(),
                mime_type: "text/plain".into(),
                md5_checksum: None,
                size: None,
                modified_time: None,
                parents: None,
                trashed: None,
            },
        ));
        let first = q.pop().unwrap();
        assert_eq!(first.kind, SyncActionKind::NewDownload);
        assert_eq!(first.relative_path, "b");
    }
}
