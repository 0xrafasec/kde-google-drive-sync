//! Emit D-Bus signals for `org.kde.GDriveSync.Daemon` (session broadcast).

use zbus::Connection;

const PATH: &str = "/org/kde/GDriveSync";
const IFACE: &str = "org.kde.GDriveSync.Daemon";

/// Emit `SyncStarted(account_id, path)`.
pub async fn sync_started(conn: &Connection, account_id: &str, path: &str) -> zbus::Result<()> {
    conn.emit_signal(
        Option::<&str>::None,
        PATH,
        IFACE,
        "SyncStarted",
        &(account_id, path),
    )
    .await
}

/// Emit `SyncCompleted(account_id, path, files_synced)`.
pub async fn sync_completed(
    conn: &Connection,
    account_id: &str,
    path: &str,
    files_synced: u32,
) -> zbus::Result<()> {
    conn.emit_signal(
        Option::<&str>::None,
        PATH,
        IFACE,
        "SyncCompleted",
        &(account_id, path, files_synced),
    )
    .await
}

/// Emit `SyncError(account_id, path, error)`.
pub async fn sync_error(conn: &Connection, account_id: &str, path: &str, error: &str) -> zbus::Result<()> {
    conn.emit_signal(
        Option::<&str>::None,
        PATH,
        IFACE,
        "SyncError",
        &(account_id, path, error),
    )
    .await
}

/// Emit `ConflictDetected(local_path, conflict_copy)`.
pub async fn conflict_detected(
    conn: &Connection,
    local_path: &str,
    conflict_copy: &str,
) -> zbus::Result<()> {
    conn.emit_signal(
        Option::<&str>::None,
        PATH,
        IFACE,
        "ConflictDetected",
        &(local_path, conflict_copy),
    )
    .await
}

/// Emit `StatusChanged(new_status)`.
pub async fn status_changed(conn: &Connection, new_status: &str) -> zbus::Result<()> {
    conn.emit_signal(
        Option::<&str>::None,
        PATH,
        IFACE,
        "StatusChanged",
        &(new_status,),
    )
    .await
}
