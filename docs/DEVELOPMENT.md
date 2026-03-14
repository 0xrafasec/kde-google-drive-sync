# Development Guide

## Prerequisites

### Rust Toolchain

```bash
# Install rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install stable + tools
rustup toolchain install stable
rustup component add clippy rustfmt

# Verify
cargo --version   # cargo 1.8x+
```

### System Dependencies

#### Fedora / RHEL

```bash
sudo dnf install \
  pkg-config \
  openssl-devel \
  sqlite-devel \
  dbus-devel \
  libsecret-devel \
  # KDE/KIO worker (optional for MVP):
  kf6-kio-devel \
  kf6-extra-cmake-modules \
  cmake \
  gcc-c++
```

#### Arch Linux

```bash
sudo pacman -S \
  pkg-config \
  openssl \
  sqlite \
  dbus \
  libsecret \
  # KDE/KIO worker (optional):
  kio \
  extra-cmake-modules \
  cmake \
  gcc
```

#### Ubuntu 24.04+

```bash
sudo apt install \
  pkg-config \
  libssl-dev \
  libsqlite3-dev \
  libdbus-1-dev \
  libsecret-1-dev \
  # KDE/KIO worker (optional):
  libkf6kio-dev \
  extra-cmake-modules \
  cmake \
  g++
```

## Project Initialization

```bash
# Clone / enter the project
cd ~/Projects/Personal/google-drive-sync

# Initialize workspace
cat > Cargo.toml << 'EOF'
[workspace]
resolver = "2"
members = [
    "crates/gds-core",
    "crates/gds-daemon",
    "crates/gds-cli",
    "crates/gds-kde",
]

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", features = ["json", "stream", "rustls-tls"], default-features = false }
zbus = { version = "4", features = ["tokio"] }
sqlx = { version = "0.8", features = ["sqlite", "runtime-tokio", "macros", "migrate"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
thiserror = "1"
anyhow = "1"
keyring = { version = "3", features = ["linux-secret-service-rt-tokio-crypto-openssl"] }
secrecy = { version = "0.8", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
clap = { version = "4", features = ["derive"] }
notify = "6"
dirs = "5"
chrono = { version = "0.4", features = ["serde"] }
EOF

# Create crate skeletons
cargo new --lib crates/gds-core
cargo new crates/gds-daemon
cargo new crates/gds-cli
cargo new crates/gds-kde

# Build to verify setup
cargo build --workspace
```

## Development Workflow

### Daily Commands

```bash
# Build everything
cargo build --workspace

# Run daemon with debug logging
RUST_LOG=gds_daemon=debug,gds_core=debug cargo run -p gds-daemon

# Run CLI
cargo run -p gds-cli -- status
cargo run -p gds-cli -- accounts list
cargo run -p gds-cli -- sync pause
cargo run -p gds-cli -- sync resume

# Run tray UI (requires KDE session)
cargo run -p gds-kde

# Lint (required before PR)
cargo clippy --workspace -- -D warnings

# Format
cargo fmt --all

# Test
cargo test --workspace
cargo test --workspace -- --nocapture  # with output

# Test specific crate
cargo test -p gds-core

# E2E tests (requires real Google account)
RUN_E2E=1 cargo test --test e2e
```

### Mock API for Development

During development, point to a local mock server instead of Google:

```bash
# Start mock server (wiremock-based, see tests/mock-server/)
cargo run -p mock-server &

# Run daemon against mock
GDS_MOCK_API=http://localhost:8080 RUST_LOG=debug cargo run -p gds-daemon
```

### Database Inspection

```bash
# SQLite DB location
DB_PATH="$HOME/.local/share/gds/state.db"

# Inspect with sqlite3
sqlite3 "$DB_PATH" ".tables"
sqlite3 "$DB_PATH" "SELECT * FROM sync_folders;"
sqlite3 "$DB_PATH" "SELECT relative_path, sync_state FROM file_states LIMIT 20;"

# Or use a GUI: sqlitebrowser
sqlitebrowser "$DB_PATH"
```

### D-Bus Debugging

```bash
# Watch all D-Bus traffic from our service
dbus-monitor --session "sender='org.kde.GDriveSync'"

# Introspect the service
dbus-send --session --print-reply \
  --dest=org.kde.GDriveSync \
  /org/kde/GDriveSync \
  org.freedesktop.DBus.Introspectable.Introspect

# Call a method
dbus-send --session --print-reply \
  --dest=org.kde.GDriveSync \
  /org/kde/GDriveSync \
  org.kde.GDriveSync.Daemon.GetStatus

# Monitor signals
dbus-monitor --session interface=org.kde.GDriveSync.Daemon
```

### Log Management

```bash
# Follow daemon logs (if using systemd)
journalctl --user -u gds-daemon -f

# Filter by level
RUST_LOG=warn cargo run -p gds-daemon

# Module-level filtering
RUST_LOG=gds_core::api=trace,gds_daemon=info cargo run -p gds-daemon
```

## Testing Guide

### Unit Tests

Pure logic tests with no I/O. Live alongside the source code.

```rust
// crates/gds-core/src/sync/diff.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conflict_detection() {
        let local = FileState {
            local_md5: Some("abc123".into()),
            drive_md5: Some("abc123".into()),  // same as last known server
            local_modified: 1000,
        };
        let remote = FileChange {
            md5: "xyz789".into(),  // server changed
            modified: 2000,
        };
        assert!(detect_conflict(&local, &remote).is_none()); // no local change, safe to download
    }
}
```

### Integration Tests

Test with real SQLite, mock HTTP. Located in `tests/integration/`.

```rust
// tests/integration/sync_test.rs
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};

#[tokio::test]
async fn test_list_files() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/drive/v3/files"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "files": [
                {"id": "file1", "name": "test.txt", "mimeType": "text/plain"}
            ]
        })))
        .mount(&mock_server)
        .await;

    let client = DriveClient::new_with_base_url(mock_server.uri(), "fake-token");
    let files = client.list_files(Default::default()).await.unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0].name, "test.txt");
}
```

## Cursor IDE Setup

### Recommended Extensions

- `rust-lang.rust-analyzer` — Rust LSP (essential)
- `tamasfe.even-better-toml` — Cargo.toml support
- `vadimcn.vscode-lldb` — Debugger
- `usernamehw.errorlens` — Inline errors
- `serayuzgur.crates` — Crate version hints

### `.cursor/settings.json`

```json
{
  "rust-analyzer.check.command": "clippy",
  "rust-analyzer.check.extraArgs": ["--", "-D", "warnings"],
  "rust-analyzer.cargo.features": "all",
  "editor.formatOnSave": true,
  "[rust]": {
    "editor.defaultFormatter": "rust-lang.rust-analyzer"
  }
}
```

### Launch Configurations (`.cursor/launch.json`)

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "type": "lldb",
      "request": "launch",
      "name": "Debug gds-daemon",
      "cargo": { "args": ["build", "-p", "gds-daemon"] },
      "args": [],
      "env": {
        "RUST_LOG": "debug",
        "GDS_MOCK_API": "http://localhost:8080"
      }
    },
    {
      "type": "lldb",
      "request": "launch",
      "name": "Debug gds-cli status",
      "cargo": { "args": ["build", "-p", "gds-cli"] },
      "args": ["status"]
    }
  ]
}
```

## CI / CD (GitHub Actions skeleton)

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - run: sudo apt-get install -y libdbus-1-dev libsecret-1-dev libssl-dev
      - run: cargo fmt --all -- --check
      - run: cargo clippy --workspace -- -D warnings
      - run: cargo test --workspace
      - run: cargo audit
```

## Packaging

### Flatpak (recommended for cross-distro distribution)

```yaml
# packaging/flatpak/org.kde.GDriveSync.yml
app-id: org.kde.GDriveSync
runtime: org.kde.Platform
runtime-version: '6.6'
sdk: org.kde.Sdk
command: gds-kde
finish-args:
  - --share=network
  - --share=ipc
  - --socket=wayland
  - --socket=fallback-x11
  - --filesystem=home
  - --talk-name=org.freedesktop.Notifications
  - --talk-name=org.kde.StatusNotifierWatcher
  - --talk-name=org.freedesktop.secrets
modules:
  - name: google-drive-sync
    buildsystem: simple
    build-commands:
      - cargo build --release --workspace
      - install -Dm755 target/release/gds-daemon /app/bin/gds-daemon
      - install -Dm755 target/release/gds-kde /app/bin/gds-kde
      - install -Dm755 target/release/gds-cli /app/bin/gds-cli
```

### RPM spec skeleton: `packaging/rpm/google-drive-sync.spec`
### Debian rules: `packaging/deb/`
### Arch PKGBUILD: `packaging/arch/PKGBUILD`

## Common Issues

### libsecret not found

```bash
# If keyring fails to link
pkg-config --libs libsecret-1
# If empty, install:
sudo dnf install libsecret-devel  # Fedora
```

### D-Bus service not starting

```bash
# Check if another instance is running
dbus-send --session --print-reply --dest=org.freedesktop.DBus \
  / org.freedesktop.DBus.ListNames | grep GDrive

# Kill it
pkill gds-daemon
```

### OAuth redirect not working

The auth flow opens a browser and listens on `localhost:8765`. Ensure:
1. Port 8765 is free
2. A default browser is configured (`xdg-settings get default-web-browser`)
3. Firewall allows local loopback
