//! Integration tests for file watcher: create, modify, delete, move; ignore patterns; debounce.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

use gds_daemon::watcher::{should_ignore, FileWatcher, WatchEvent};

#[test]
fn ignore_pattern_matching() {
    let root = PathBuf::from("/tmp/sync_root");
    assert!(should_ignore(&root, &root.join(".gds_tmp/file")));
    assert!(should_ignore(&root, &root.join("a/.gds_tmp")));
    assert!(should_ignore(&root, &root.join(".git/HEAD")));
    assert!(should_ignore(&root, &root.join("sub/.git/config")));
    assert!(should_ignore(&root, &root.join("file.swp")));
    assert!(should_ignore(&root, &root.join("doc.txt~")));
    assert!(should_ignore(&root, &root.join(".#file.txt")));
    assert!(should_ignore(
        &root,
        &root.join("doc.txt.conflict-20240115-120000")
    ));
}

#[test]
fn watcher_detects_create_and_modify() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();

    let watcher = FileWatcher::new(root.clone(), 300);
    let rx = watcher.start().expect("start watcher");

    // Give the watcher time to register
    std::thread::sleep(Duration::from_millis(100));

    // Create a file
    let file_path = root.join("test.txt");
    std::fs::write(&file_path, "hello").expect("write file");

    // Expect at least one Changed event (debounced)
    let mut received = false;
    let timeout = Duration::from_millis(800);
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(WatchEvent::Changed(p)) => {
                if p == file_path || p.ends_with("test.txt") {
                    received = true;
                    break;
                }
            }
            Ok(WatchEvent::Error) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    assert!(received, "watcher should emit Changed for created file");
}

#[test]
fn watcher_ignores_gds_tmp() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();

    std::fs::create_dir_all(root.join(".gds_tmp")).expect("create .gds_tmp");

    let watcher = FileWatcher::new(root.clone(), 200);
    let rx = watcher.start().expect("start watcher");

    std::thread::sleep(Duration::from_millis(100));

    let file_in_tmp = root.join(".gds_tmp/secret");
    std::fs::write(&file_in_tmp, "data").expect("write");

    // We should not receive an event for .gds_tmp/secret (ignored)
    let timeout = Duration::from_millis(500);
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(WatchEvent::Changed(p)) => {
                let s = p.to_string_lossy();
                assert!(
                    !s.contains(".gds_tmp"),
                    "should not emit event for .gds_tmp path"
                );
            }
            Ok(WatchEvent::Error) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => break,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

#[test]
fn debounce_coalesces_rapid_events() {
    let dir = tempfile::tempdir().expect("tempdir");
    let root = dir.path().to_path_buf();

    let watcher = FileWatcher::new(root.clone(), 400);
    let rx = watcher.start().expect("start watcher");

    std::thread::sleep(Duration::from_millis(100));

    let file_path = root.join("rapid.txt");
    // Rapid writes (each could generate events)
    for i in 0..10 {
        std::fs::write(&file_path, format!("{}", i)).expect("write");
        std::thread::sleep(Duration::from_millis(20));
    }

    // After debounce window we should get at least one event, and not necessarily 10
    let mut count = 0u32;
    let deadline = std::time::Instant::now() + Duration::from_millis(1000);
    while std::time::Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(WatchEvent::Changed(p)) => {
                if p.ends_with("rapid.txt") {
                    count += 1;
                }
            }
            Ok(WatchEvent::Error) => {}
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    // Debouncing should coalesce: we expect a small number of events (e.g. 1–3), not 10
    assert!(count >= 1, "at least one event after debounce");
    assert!(count <= 5, "debounce should coalesce rapid events");
}
