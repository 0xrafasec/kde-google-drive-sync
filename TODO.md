# Development Roadmap

Legend: `[ ]` not started · `[~]` in progress · `[x]` complete

Each item must be **fully implemented with tests** before being marked `[x]`.
No partial work, no placeholders. See `CLAUDE.md` → Production-Ready Code Standard.

---

## Phase 1 — Core Library (`gds-core`)

Foundation. Everything else depends on this being solid.

### 1.1 Domain Model

- [x] `model::DriveFile` — id, name, mimeType, md5Checksum, size, modifiedTime, parents, trashed
- [x] `model::Account` — id, email, displayName, keyringKey, createdAt
- [x] `model::SyncFolder` — id, accountId, localPath, driveFolderId, startPageToken, lastSyncAt, paused
- [x] `model::FileState` — id, syncFolderId, relativePath, driveFileId, driveMd5, driveModified, localMd5, localModified, syncState, lastSyncedAt
- [x] `model::SyncState` enum — Synced, Pending, Conflict, Error, Uploading, Downloading
- [x] `model::SyncError` — typed error enum with thiserror (all variants: QuotaExceeded, Conflict, PathTraversal, AuthExpired, NetworkError, IoError, DatabaseError, ApiError)
- [x] `model::ConflictInfo` — localPath, conflictCopyPath, serverVersion, localVersion
- [x] `model::ChangeSet` — list of Drive changes from changes.list API
- [x] `model::Config` — all configurable values with serde Deserialize + Default impl
- [x] Unit tests for all model types (construction, serialization round-trip)

### 1.2 Database Layer (`gds-core::db`)

- [x] SQLite connection pool setup (sqlx, WAL mode, busy timeout)
- [x] Migration system (`sqlx::migrate!` with versioned migration files)
- [x] Migration 001: create accounts table
- [x] Migration 002: create sync_folders table
- [x] Migration 003: create file_states table
- [x] Migration 004: create sync_errors table
- [x] `AccountRepository` — insert, get_by_id, list_all, delete
- [x] `SyncFolderRepository` — insert, get_by_id, list_by_account, delete, set_paused, update_page_token
- [x] `FileStateRepository` — upsert, get_by_path, list_by_folder, list_by_state, delete, bulk_upsert
- [x] `SyncErrorRepository` — insert, get_recent, clear_for_file, increment_retry
- [x] Transaction support for multi-table operations (e.g., account delete cascades)
- [x] Integration tests for every repository method (real SQLite in-memory)

### 1.3 Authentication (`gds-core::auth`)

- [x] OAuth2 client using `oauth2` crate (full PKCE flow for desktop app)
- [x] Loopback redirect server (listen on `127.0.0.1:PORT`, parse code+state)
- [x] Random port selection with fallback list if port is in use
- [x] CSRF state parameter generation and validation
- [x] Auth code → token exchange (access_token + refresh_token)
- [x] Token refresh logic (refresh before expiry, retry on 401)
- [x] `TokenStore` trait with `libsecret` implementation (store/load/delete refresh_token)
- [x] `TokenStore` implementation for KWallet (fallback)
- [x] `TokenStore` implementation for in-memory (tests only)
- [x] Automatic token refresh integrated into all API calls (transparent to callers)
- [x] `xdg-open` browser launch with error handling (fallback: print URL to stdout)
- [x] Full auth flow integration test using mock OAuth server
- [x] Token revocation on account removal

### 1.4 Google Drive API Client (`gds-core::api`)

- [x] `DriveClient` struct (reqwest client, base URL injectable for testing)
- [x] `files.list` — paginated, with `q` filter, `fields` partial response, `orderBy`
- [x] `files.get` — metadata only, with fields selection
- [x] `files.get?alt=media` — download file content to `AsyncWrite`
- [x] `files.export` — export Google Workspace files (Docs→docx, Sheets→xlsx, etc.)
- [x] `files.create` (simple upload, ≤5 MB, multipart)
- [x] `files.create` (resumable upload, >5 MB, chunked with progress callback)
- [x] `files.update` (metadata only: rename, move, trash)
- [x] `files.update` (content, simple upload)
- [x] `files.update` (content, resumable upload with resume-from-offset on failure)
- [x] `files.delete` (permanent delete)
- [x] `files.copy` (server-side copy)
- [x] `changes.getStartPageToken`
- [x] `changes.list` (paginated, stores nextPageToken, detects newStartPageToken)
- [x] `about.get` (quota information: usage, limit)
- [x] `drive.list` (shared drives support — future, but stub the interface now)
- [x] Exponential backoff with jitter for all retryable errors (429, 500, 502, 503, 504)
- [x] Retry-After header parsing for 429 responses
- [x] Per-request timeout (30s default, 5 min for large uploads)
- [x] Request/response tracing at TRACE level (with token redaction)
- [x] Google Workspace MIME type detection and export routing
- [x] Integration tests: all methods against wiremock mock server
- [x] Test: correct backoff timing under rate limiting
- [x] Test: resumable upload resume after simulated mid-upload failure

### 1.5 Sync Engine (`gds-core::sync`)

- [x] `DiffEngine` — compare local filesystem state vs. SQLite known state vs. Drive changes
- [x] `DiffEngine::compute_local_changes` — walk local dir, compute MD5, compare to DB
- [x] `DiffEngine::compute_remote_changes` — process Drive changeset, compare to DB
- [x] Change classification: new upload, new download, update upload, update download, delete local, delete remote, conflict
- [x] Conflict detection: both local and remote changed since last sync
- [x] Conflict resolution: server wins + local copy saved as `.conflict-YYYYMMDD-HHMMSS`
- [x] Conflict copy naming: collision-safe (append `-2`, `-3` if conflict copy already exists)
- [x] `SyncQueue` — priority queue for pending sync operations (downloads before uploads for initial sync)
- [x] `SyncExecutor` — runs queued operations with concurrency limit (N uploads, M downloads)
- [x] Atomic download: write to temp file, fsync, atomic rename
- [x] Path validation: `safe_local_path()` prevents traversal attacks (see SECURITY.md)
- [x] Symlink policy: skip external symlinks during upload scan, never create symlinks on download
- [x] Google Workspace stub files: create `.gdoc`, `.gsheet`, `.gslides` shortcut files (contain URL)
- [x] Binary file deduplication: skip upload if local MD5 matches last known server MD5
- [x] Large directory handling: streaming walk, not load-all-into-memory
- [x] Initial sync: full recursive scan + reconcile (must handle 100k+ files without OOM)
- [x] Incremental sync: changes.list-based, efficient (only process delta)
- [x] Pause/resume: `SyncExecutor` checks pause flag between operations
- [x] Unit tests: DiffEngine with all change type combinations
- [x] Unit tests: conflict detection matrix (local changed / remote changed / both / neither)
- [x] Unit tests: path validation with adversarial inputs (`../../../etc/passwd`, absolute paths, null bytes)
- [x] Integration test: full sync cycle (upload + download + conflict) against mock Drive API

---

## Phase 2 — Daemon (`gds-daemon`)

### 2.1 File Watcher (`gds-daemon::watcher`)

- [x] `FileWatcher` using `notify` crate (inotify backend on Linux)
- [x] Recursive watch on all configured sync folders
- [x] Watch new subdirectories as they are created (dynamic watch add)
- [x] Event debouncing: 500ms window to coalesce rapid writes (e.g., editor save)
- [x] Ignore patterns: `.gds_tmp`, `.git/`, common editor temp files (`*.swp`, `~*`, `.#*`)
- [x] Ignore own writes: don't re-trigger sync on files written by the daemon itself
- [x] Watcher recovery: re-establish watches after `IN_MOVE_SELF` or watch fd invalidation
- [x] Unit test: debounce logic (rapid events → single notification)
- [x] Unit test: ignore pattern matching
- [x] Integration test: watcher detects create, modify, delete, move events

### 2.2 D-Bus Service (`gds-daemon::dbus`)

- [x] Register `org.kde.GDriveSync` on session bus using zbus
- [x] Implement `GetStatus() → (status: String, syncing_count: u32)` — fully accurate
- [x] Implement `PauseSync()` — pauses all sync queues, persists state to DB
- [x] Implement `ResumeSync()` — resumes all queues
- [x] Implement `ForceSync(path: String)` — immediate sync of specific path
- [x] Implement `GetAccounts() → Array<AccountInfo>` — live data from DB
- [x] Implement `AddAccount()` — triggers full OAuth flow, blocks until complete or error
- [x] Implement `RemoveAccount(id: String)` — stops sync, deletes DB records, revokes token, removes keyring entry
- [x] Implement `GetSyncFolders() → Array<SyncFolder>`
- [x] Implement `AddSyncFolder(local_path, drive_folder_id)` — validates paths, starts initial sync
- [x] Implement `RemoveSyncFolder(id)` — stops sync for folder, removes DB records (does NOT delete local files)
- [x] Implement `GetSyncErrors() → Array<SyncErrorInfo>` — recent errors per account
- [x] Implement `GetAboutInfo(account_id) → QuotaInfo` — Drive quota from API
- [x] Emit `SyncStarted(account_id, path)` signal
- [x] Emit `SyncCompleted(account_id, path, files_synced)` signal
- [x] Emit `SyncError(account_id, path, error)` signal
- [x] Emit `ConflictDetected(local_path, conflict_copy)` signal
- [x] Emit `StatusChanged(new_status)` signal
- [x] D-Bus introspection XML generated and shipped as asset
- [x] Integration test: call every method via zbus test client

### 2.3 Scheduler (`gds-daemon::scheduler`)

- [x] Poll scheduler: run `changes.list` per account on configurable interval (default 30s)
- [x] Event-driven trigger: file watcher events immediately queue a sync
- [x] Rate limiter: max N sync operations per second (token bucket)
- [x] Upload queue: max 2 concurrent uploads (configurable)
- [x] Download queue: max 4 concurrent downloads (configurable)
- [x] Retry queue: failed operations re-queued with exponential backoff
- [x] Backoff state persisted to DB (survives daemon restart)
- [x] Graceful shutdown: finish in-flight operations, flush DB, deregister D-Bus
- [x] Unit test: token bucket rate limiter correctness
- [x] Unit test: retry queue backoff timing

### 2.4 Daemon Bootstrap

- [x] `main.rs`: parse CLI args (config path override, log level, foreground flag)
- [x] Load and validate config from `~/.config/gds/config.toml`
- [x] Initialize SQLite (run migrations)
- [x] Initialize all accounts from DB (re-establish token refresh for each)
- [x] Register D-Bus service (fail fast if already registered — single instance enforcement)
- [x] Initialize file watchers for all active sync folders
- [x] Start scheduler
- [x] Handle SIGTERM/SIGINT for graceful shutdown
- [x] Write PID file to `~/.local/share/gds/daemon.pid`
- [x] systemd user unit file: `packaging/systemd/gds-daemon.service`

---

## Phase 3 — CLI (`gds-cli`)

All commands communicate with the daemon via D-Bus. No direct DB access.

- [ ] `gds status` — print per-account sync status, quota, last sync time
- [ ] `gds accounts list` — list configured accounts (email, id, status)
- [ ] `gds accounts add` — trigger OAuth flow via daemon
- [ ] `gds accounts remove <id>` — remove account with confirmation prompt
- [ ] `gds sync pause` — pause all sync
- [ ] `gds sync resume` — resume sync
- [ ] `gds sync now [path]` — force immediate sync (optionally: specific path)
- [ ] `gds folders list` — list sync folder mappings
- [ ] `gds folders add <local-path> <drive-folder-id>` — add sync folder
- [ ] `gds folders remove <id>` — remove sync folder mapping
- [ ] `gds errors` — show recent sync errors
- [ ] `gds quota` — show Drive storage quota per account
- [ ] `gds daemon start` — start daemon if not running
- [ ] `gds daemon stop` — graceful daemon shutdown
- [ ] `gds daemon status` — is daemon running? PID?
- [ ] `--json` global flag for machine-readable output (all commands)
- [ ] `--quiet` / `--verbose` flags
- [ ] Proper exit codes (0=success, 1=error, 2=daemon not running)
- [ ] Man page generated from clap (`clap_mangen`)
- [ ] Shell completions for bash, zsh, fish (`clap_complete`)
- [ ] Integration tests: all commands (daemon mocked via D-Bus test double)

---

## Phase 4 — KDE UI (`gds-kde`)

### 4.1 System Tray

- [ ] SNI tray registration via `ksni` crate
- [ ] State-driven icon: idle (green check), syncing (spinner), paused, error, offline
- [ ] Icon set: hicolor at 16, 22, 32, 48, 64, 128px (SVG source)
- [ ] Tooltip: account name + status + last sync time
- [ ] Context menu: full menu as specified in `docs/KDE_INTEGRATION.md`
- [ ] Menu items: Open Folder, Open in Browser, Pause/Resume (toggle), Force Sync, Preferences, Activity Log, Quit
- [ ] Real-time status update: subscribe to D-Bus `StatusChanged` signal
- [ ] Multiple accounts: sub-menu per account if >1 configured
- [ ] "Open in Browser" opens `https://drive.google.com` with `xdg-open`
- [ ] Preferences dialog (basic): sync interval, notification settings, conflict policy
- [ ] Activity log window: scrollable list of recent sync events (last 500)
- [ ] Unit test: menu item actions trigger correct D-Bus calls
- [ ] Manual test checklist in `docs/KDE_INTEGRATION.md`

### 4.2 Notifications

- [ ] `NotificationManager` subscribes to all D-Bus signals from daemon
- [ ] Notification: sync complete (batched — one notification per sync cycle, not per file)
- [ ] Notification: conflict detected with action buttons ("Keep Mine", "View Diff", "Dismiss")
- [ ] Notification: auth required (token expired) with "Sign In" action
- [ ] Notification: sync error (persistent, with "Retry" action)
- [ ] Notification: low Drive quota warning (<10% free)
- [ ] Notification: initial sync started (first-run only)
- [ ] Notification deduplication: don't spam the same error repeatedly
- [ ] "View Diff" action: launch KDiff3 if installed, fallback to meld, fallback to diff in terminal
- [ ] Integration test: notification sent for each signal type

---

## Phase 5 — KIO Worker (`kio-worker`)

Dolphin integration. C++ thin layer only — all logic in daemon.

- [ ] `gdrivekio.cpp`: full `KIO::WorkerBase` subclass
- [ ] `listDir(url)` — calls `ListDir` D-Bus method, emits `UDSEntry` for each item
- [ ] `stat(url)` — calls `Stat` D-Bus method, emits `UDSEntry`
- [ ] `get(url)` — calls `GetFileContent` D-Bus method, streams data, sets MIME type
- [ ] `put(url, permissions, flags)` — calls `UploadFile` D-Bus method, streams data
- [ ] `del(url, isfile)` — calls `DeleteItem` D-Bus method
- [ ] `mkdir(url, permissions)` — calls `CreateDirectory` D-Bus method
- [ ] `copy(src, dst, permissions, flags)` — calls `CopyItem` D-Bus method (server-side copy)
- [ ] `rename(src, dst, flags)` — calls `MoveItem` D-Bus method (server-side rename)
- [ ] Progress reporting: `processedSize`, `totalSize` signals during get/put
- [ ] Error mapping: Drive API errors → KIO error codes
- [ ] `gdrive.protocol` file with correct metadata
- [ ] Add `ListDir`, `Stat`, `GetFileContent`, `UploadFile`, `DeleteItem`, `CreateDirectory`, `CopyItem`, `MoveItem` D-Bus methods to daemon interface (extends Phase 2.2)
- [ ] CMakeLists.txt: builds cleanly against KF6, no warnings
- [ ] Installable to `${KDE_INSTALL_PLUGINDIR}/kf6/kio`
- [ ] Manual test: `kioclient6 ls gdrive:/` works
- [ ] Manual test: Dolphin can browse, open, copy, delete files

---

## Phase 6 — Packaging

All three formats must be completed before any release is tagged.

### 6.1 RPM (`packaging/rpm/`)

- [ ] `google-drive-sync.spec` — complete spec file
  - [ ] BuildRequires: all Rust + KDE build deps
  - [ ] %prep: source setup
  - [ ] %build: `cargo build --release --workspace` + `cmake` for KIO worker
  - [ ] %install: install all binaries, systemd unit, .desktop, icons, protocol file
  - [ ] %files: complete file list with correct ownership
  - [ ] %changelog: maintained
  - [ ] Scriptlets: `%post`/`%preun` for systemd unit enable/disable
- [ ] Builds cleanly with `rpmbuild` on Fedora 41+
- [ ] Installs and runs correctly on clean Fedora VM
- [ ] Submitted to Fedora COPR

### 6.2 DEB (`packaging/deb/`)

- [ ] `control` — package metadata, complete Depends/Build-Depends
- [ ] `rules` — `dh` based, builds Rust + CMake correctly
- [ ] `changelog` — properly formatted Debian changelog
- [ ] `copyright` — DEP-5 machine-readable copyright
- [ ] `install` — files to install list
- [ ] `gds-daemon.service` symlink for dh_systemd
- [ ] Builds cleanly with `dpkg-buildpackage` on Ubuntu 24.04
- [ ] Installs and runs correctly on clean Ubuntu VM
- [ ] Submitted to Launchpad PPA

### 6.3 Arch (`packaging/arch/`)

- [ ] `PKGBUILD` — complete, follows Arch packaging guidelines
  - [ ] `pkgname`, `pkgver`, `pkgrel`, `arch`, `depends`, `makedepends`
  - [ ] `source` array with integrity checksums
  - [ ] `build()` function
  - [ ] `package()` function
- [ ] Builds cleanly with `makepkg` on Arch
- [ ] Submitted to AUR

### 6.4 Flatpak (`packaging/flatpak/`)

- [ ] `org.kde.GDriveSync.yml` — complete Flatpak manifest
  - [ ] Runtime: `org.kde.Platform//6.8`
  - [ ] All finish-args (network, filesystem, D-Bus names)
  - [ ] Rust sources via `cargo-sources.json` (generated by `flatpak-cargo-generator.py`)
  - [ ] KIO worker built with flatpak CMake module
  - [ ] All binaries installed to `/app/bin/`
  - [ ] Icons installed to `/app/share/icons/`
  - [ ] .desktop installed to `/app/share/applications/`
- [ ] Builds with `flatpak-builder` producing a valid bundle
- [ ] Runs correctly in Flatpak sandbox (all D-Bus permissions work)
- [ ] Submitted to Flathub

### 6.5 Release Automation

- [ ] `scripts/make-release.sh` — tags git, bumps version in Cargo.toml, builds all 3 formats, generates checksums
- [ ] GitHub Actions: build RPM + DEB + Flatpak on every tag, upload as release assets
- [ ] GitHub Actions: `cargo audit` + `cargo deny` in CI

---

## Phase 7 — Quality & Polish

- [ ] `cargo audit` passes (zero high/critical CVEs)
- [ ] `cargo deny` passes (licenses, duplicates)
- [ ] All performance targets met (see CLAUDE.md)
- [ ] Security checklist from `docs/SECURITY.md` fully completed
- [ ] Memory leak testing: run daemon 24h, verify RSS stays bounded
- [ ] Stress test: sync folder with 50k files, verify correctness
- [ ] Test with slow/unreliable network (tc netem): verify no data loss
- [ ] Test token expiry mid-sync: verify transparent refresh
- [ ] Test disk-full condition during download: verify no corrupt files
- [ ] Test concurrent edits (local + remote simultaneously): conflict always detected
- [ ] Accessibility: tray menu keyboard navigation works
- [ ] Tested on: Fedora 41 KDE, Ubuntu 24.04 with KDE, Arch with KDE Plasma 6, openSUSE Tumbleweed

---

## Deferred (Post-1.0)

These are explicitly out of scope for 1.0. Do not implement prematurely.

- Drive push notifications (webhooks) — polling is sufficient for 1.0
- Shared Drives / Team Drives support
- Selective sync (choose individual folders)
- Bandwidth throttling
- Google Photos sync
- GNOME port (gds-gnome crate)
- Plasma widget (full widget, beyond system tray)
- Windows port
- macOS port
- Conflict resolution UI (KDiff3 launch is sufficient for 1.0)
- File versioning / history viewer
