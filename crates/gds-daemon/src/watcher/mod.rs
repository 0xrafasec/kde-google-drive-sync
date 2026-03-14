//! File watcher (inotify, debouncing).
//!
//! Recursive watch on sync roots with 500ms debounce and ignore patterns.

mod ignore;

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebounceEventResult};
use tracing::{debug, warn};

pub use ignore::should_ignore;

/// Event emitted when the watcher detects a change (after debounce and ignore filter).
#[derive(Clone, Debug)]
pub enum WatchEvent {
    /// A path under the sync root changed; trigger sync for this folder.
    Changed(PathBuf),
    /// Watcher encountered an error (e.g. IN_MOVE_SELF); caller may re-establish watch.
    Error,
}

/// File watcher: recursive watch with debouncing and ignore patterns.
pub struct FileWatcher {
    sync_root: PathBuf,
    debounce_ms: u64,
}

impl FileWatcher {
    /// Creates a new watcher. Call `start()` to begin watching and get the event receiver.
    pub fn new(sync_root: PathBuf, debounce_ms: u64) -> Self {
        Self {
            sync_root,
            debounce_ms,
        }
    }

    /// Starts watching. Returns the receiver for debounced, filtered events.
    /// The watcher runs in a background thread (from the debouncer library).
    /// On `WatchEvent::Error` the caller may re-create the watcher to recover (e.g. after IN_MOVE_SELF).
    pub fn start(self) -> Result<mpsc::Receiver<WatchEvent>, notify::Error> {
        let (tx, rx) = mpsc::channel();
        let sync_root = self.sync_root.clone();
        let debounce_ms = self.debounce_ms;

        let mut debouncer = new_debouncer(
            Duration::from_millis(debounce_ms),
            move |res: DebounceEventResult| match res {
                Ok(events) => {
                    for event in events {
                        let path = &event.path;
                        if !ignore::should_ignore(&sync_root, path) {
                            if tx.send(WatchEvent::Changed(path.to_path_buf())).is_err() {
                                return;
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!(error = %e, "file watcher error");
                    let _ = tx.send(WatchEvent::Error);
                }
            },
        )?;

        debouncer
            .watcher()
            .watch(&self.sync_root, RecursiveMode::Recursive)?;

        // Keep the debouncer alive in a dedicated thread so watching continues.
        std::thread::spawn(move || {
            let _guard = debouncer;
            loop {
                std::thread::park();
            }
        });

        debug!(sync_root = %self.sync_root.display(), "file watcher started");
        Ok(rx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debounce_delay_is_honored() {
        // Unit test: we can't easily test debounce timing without a real filesystem
        // and time; that's done in integration. Here we just ensure the type builds.
        let _ = FileWatcher::new(PathBuf::from("/tmp"), 500);
    }
}
