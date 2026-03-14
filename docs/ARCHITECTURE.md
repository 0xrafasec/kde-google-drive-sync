# Architecture

## System Overview

```mermaid
flowchart TB
  subgraph shell["KDE Plasma Shell"]
    tray["System Tray (gds-kde)"]
    dolphin["Dolphin (KIO Worker)"]
    notif["KDE Notifications (org.freedesktop)"]
  end

  subgraph daemon["gds-daemon (systemd user unit)"]
    dbus["D-Bus Service (zbus)"]
    watcher["File Watcher (inotify)"]
    sched["Scheduler (rate limit, backoff)"]
    subgraph core["gds-core"]
      sync["Sync Engine (diff, conflict)"]
      api["Drive API Client (OAuth2, HTTP)"]
      db["Local DB (SQLite/sqlx)"]
    end
  end

  drive["Google Drive API v3 (HTTPS)"]
  keyring["libsecret / KWallet (OAuth tokens)"]

  tray -->|D-Bus| dbus
  dolphin -->|KIO protocol| dbus
  notif -->|D-Bus| dbus
  dbus --> core
  watcher --> core
  sched --> core
  core --> drive
  core --> keyring
```

## Sync Engine State Machine

```mermaid
stateDiagram-v2
  [*] --> IDLE
  IDLE --> SCANNING: file event / poll timer
  SCANNING --> SYNCING: diff ready
  SCANNING --> IDLE: no changes
  SYNCING --> IDLE: done
  SYNCING --> CONFLICT: both sides changed
  CONFLICT --> IDLE: user choice
  SYNCING --> ERROR: failure
  ERROR --> IDLE: backoff retry
  ERROR --> IDLE: give up
```

## Conflict Resolution Policy

**Default: Server wins, local copy preserved**

```
Local:  file.txt  (modified at T2)
Server: file.txt  (modified at T3, T3 > T2)

Result:
  ~/GDrive/file.txt                          ← server version
  ~/GDrive/file.conflict-20260313-143022.txt ← local version
```

User is notified via KDE notification with action buttons:
- "Keep mine" → re-uploads local conflict copy
- "View diff" → opens KDiff3 if installed
- "Dismiss" → deletes conflict copy

## Data Flow: Upload

```mermaid
flowchart LR
  A[inotify CLOSE_WRITE] --> B[Debounce 500ms]
  B --> C[Compute local hash]
  C --> D[Query SQLite]
  D --> E{Hash changed?}
  E -->|No| F[Skip]
  E -->|Yes| G[Enqueue upload]
  G --> H[Drive API: files.get]
  H --> I{Server newer?}
  I -->|Yes| J[Conflict]
  I -->|No| K[Upload file]
  K --> L[Update SQLite]
  L --> M[Emit SyncCompleted]
```

## Data Flow: Download

```mermaid
flowchart LR
  A[Scheduler / poll 30s] --> B[changes.list]
  B --> C[Fetch changeset]
  C --> D[For each change]
  D --> E{Local modified?}
  E -->|Yes| F[Conflict]
  E -->|No| G[Download to temp]
  G --> H[Atomic rename]
  H --> I[Update SQLite]
  I --> J[Store new page token]
  J --> K[Emit D-Bus signals]
```

## SQLite Schema

```sql
CREATE TABLE accounts (
    id          TEXT PRIMARY KEY,  -- UUID
    email       TEXT NOT NULL,
    display_name TEXT,
    keyring_key TEXT NOT NULL,     -- key name in libsecret
    created_at  INTEGER NOT NULL   -- unix timestamp
);

CREATE TABLE sync_folders (
    id              TEXT PRIMARY KEY,  -- UUID
    account_id      TEXT NOT NULL REFERENCES accounts(id),
    local_path      TEXT NOT NULL,
    drive_folder_id TEXT NOT NULL,
    start_page_token TEXT,             -- Drive changes token
    last_sync_at    INTEGER,
    paused          INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE file_states (
    id              TEXT PRIMARY KEY,  -- UUID
    sync_folder_id  TEXT NOT NULL REFERENCES sync_folders(id),
    relative_path   TEXT NOT NULL,
    drive_file_id   TEXT,
    drive_md5       TEXT,
    drive_modified  INTEGER,           -- unix ms
    local_md5       TEXT,
    local_modified  INTEGER,           -- unix ms
    sync_state      TEXT NOT NULL,     -- 'synced', 'pending', 'conflict', 'error'
    last_synced_at  INTEGER,
    UNIQUE(sync_folder_id, relative_path)
);

CREATE TABLE sync_errors (
    id              TEXT PRIMARY KEY,
    file_state_id   TEXT REFERENCES file_states(id),
    error_message   TEXT NOT NULL,
    occurred_at     INTEGER NOT NULL,
    retry_count     INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX idx_file_states_folder ON file_states(sync_folder_id);
CREATE INDEX idx_file_states_drive_id ON file_states(drive_file_id);
```

## Security Boundary Map

```mermaid
flowchart TB
  subgraph internet["Trust boundary: Internet"]
    I1["TLS 1.2+ only, pinning for *.googleapis.com"]
    I2["Tokens in memory only during use"]
    I3["Tokens in keyring (libsecret/KWallet), never on disk"]
  end

  subgraph fs["Trust boundary: Local filesystem"]
    F1["Sync folder 700"]
    F2["Temp files → same FS, atomic rename"]
    F3["Config dir ~/.config/gds (700)"]
  end

  subgraph dbus["Trust boundary: D-Bus"]
    D1["Session bus (user-local)"]
    D2["Policy file for sensitive methods"]
    D3["KIO worker as user, no elevation"]
  end
```

## Crate Dependency Graph

```mermaid
flowchart TB
  gds_cli["gds-cli"]
  gds_kde["gds-kde"]
  gds_daemon["gds-daemon"]
  gds_core["gds-core"]

  gds_cli --> zbus
  gds_kde --> zbus
  gds_daemon --> zbus
  gds_daemon --> gds_core
  gds_daemon --> inotify["inotify"]
  gds_daemon --> keyring["keyring"]
  gds_daemon --> tracing["tracing"]

  subgraph core_deps["gds-core dependencies"]
    reqwest
    oauth2
    sqlx
    serde
    notify
    thiserror
  end

  gds_core --> core_deps
```

## Threading Model

```mermaid
flowchart TB
  subgraph runtime["Tokio runtime"]
    t1["D-Bus service loop (zbus)"]
    t2["File watcher (notify → channel)"]
    t3["Sync scheduler (periodic + event-driven)"]
    t4["Upload queue workers (N=2)"]
    t5["Download queue workers (N=4)"]
    t6["Changes poll loop (per account)"]
  end

  runtime --> shared["Shared state: Arc<Mutex<T>> / Arc<RwLock<T>>"]
  runtime --> channels["Channels: mpsc, broadcast, oneshot"]
```

## Portability Notes

- **inotify** is Linux-only. The `notify` crate abstracts this — on macOS it
  uses FSEvents, on Windows ReadDirectoryChangesW. The sync engine depends only
  on the `notify` trait, not the platform backend.
- **KDE-specific** code is fully isolated in `gds-kde` and `kio-worker`.
  `gds-daemon` uses only standard D-Bus (freedesktop) interfaces.
- **GNOME port**: replace `gds-kde` with a GNOME shell extension or GTK4 tray app.
  Core and daemon are unchanged.
- **Static linking**: `cargo build --release` with `RUSTFLAGS='-C target-feature=+crt-static'`
  produces a mostly-static binary. Distribute via Flatpak for full isolation.
