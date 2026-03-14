# Security Design

## Threat Model

| Threat | Mitigation |
|---|---|
| Stolen OAuth token | Stored only in libsecret/KWallet, never on disk |
| MITM on Google API | TLS 1.2+, system cert store validation |
| Local privilege escalation | Daemon runs as user, no suid, no capabilities |
| Malicious file content | We sync content, not execute it — no interpretation |
| Path traversal via Drive | All paths normalized and validated against sync root |
| Symlink attacks | Symlinks not followed outside sync folder |
| Token leakage in logs | Tokens scrubbed from all log output at compile time |
| D-Bus spoofing | Session bus only (not system bus) |

## Authentication

### OAuth2 Flow

We use the **Desktop App / Installed Application** OAuth2 flow:

```
1. User triggers "Add Account" via tray or CLI
2. We request an authorization URL with scopes:
     - https://www.googleapis.com/auth/drive
     - https://www.googleapis.com/auth/userinfo.email
3. Open URL in user's default browser (xdg-open)
4. Listen on localhost:PORT for redirect (loopback redirect URI)
5. Exchange auth code for access_token + refresh_token
6. Store refresh_token in libsecret immediately
7. Discard access_token after use (re-request via refresh as needed)
8. Never log or store the auth code, access token, or refresh token in files
```

### Required OAuth Scopes

Prefer the **minimum required scope**:

| Scope | Why |
|---|---|
| `drive` | Full Drive access (required for sync) |
| `userinfo.email` | Display account in UI |

If Google approves a restricted scope for file-by-file access in the future,
migrate to `drive.file` (only files created by our app) for a less-privileged option.

### Token Storage

```rust
// SECURITY: tokens are NEVER written to disk, config files, or logs.
// Only the keyring abstraction may touch token material.

use keyring::Entry;

pub fn store_refresh_token(account_id: &str, token: &str) -> Result<()> {
    let entry = Entry::new("gds", account_id)?;
    entry.set_password(token)?;
    // token goes out of scope and is zeroed by zeroize
    Ok(())
}

pub fn load_refresh_token(account_id: &str) -> Result<String> {
    let entry = Entry::new("gds", account_id)?;
    Ok(entry.get_password()?)
}
```

Use the `secrecy` crate for in-memory token handling:
```rust
use secrecy::{Secret, ExposeSecret};

let token: Secret<String> = Secret::new(raw_token);
// Only expose when needed for HTTP header:
let header_value = format!("Bearer {}", token.expose_secret());
```

## Network Security

- **TLS**: `reqwest` uses the system's native TLS (rustls or openssl feature flag).
  Default to `rustls` for no system dependency.
- **Certificate validation**: Never disable — `danger_accept_invalid_certs` is
  forbidden in production code. A clippy custom lint should catch this.
- **Redirect policy**: Do not follow redirects from Google API endpoints to
  non-Google domains.
- **Timeout**: All HTTP requests must have a timeout (30s default, 5 min for
  resumable uploads).

## File System Security

### Path Validation

```rust
// SECURITY: Prevent path traversal from Drive file names.
// Drive allows filenames like "../../../etc/passwd".
// Always resolve relative to sync root and verify containment.

pub fn safe_local_path(sync_root: &Path, relative: &str) -> Result<PathBuf> {
    // Strip any leading / or .. components
    let sanitized = relative
        .split('/')
        .filter(|c| !c.is_empty() && *c != "..")
        .collect::<Vec<_>>()
        .join("/");

    let candidate = sync_root.join(&sanitized);
    let canonical = candidate.canonicalize()
        .unwrap_or(candidate.clone()); // file may not exist yet

    // Ensure result is still inside sync root
    if !canonical.starts_with(sync_root) {
        return Err(SyncError::PathTraversal { path: relative.to_string() });
    }
    Ok(candidate)
}
```

### Atomic Writes

Downloaded files are written to a temporary path first, then atomically renamed:

```rust
// SECURITY: Atomic write prevents partial reads of in-progress downloads.
let tmp = target.with_extension("gds_tmp");
write_to_file(&tmp, content).await?;
tokio::fs::rename(&tmp, &target).await?; // atomic on same filesystem
```

### Symlink Policy

- **Never follow symlinks** outside the sync root during upload scanning.
- **Never create symlinks** during download — if Drive returns a shortcut, skip it
  or create a `.gdoc` stub file.

```rust
let meta = tokio::fs::symlink_metadata(&path).await?;
if meta.file_type().is_symlink() {
    // Check if it points outside sync root
    let target = tokio::fs::read_link(&path).await?;
    // SECURITY: skip if external symlink
    if !target.starts_with(sync_root) {
        tracing::warn!("Skipping external symlink: {}", path.display());
        return Ok(());
    }
}
```

## D-Bus Security

- Service runs on the **session bus** (user-local), not the system bus.
- Sensitive methods (AddAccount, RemoveAccount) should require the caller to
  confirm via a polkit dialog in the future (post-MVP).
- No world-readable D-Bus policy files.

## Log Sanitization

Never log secrets. Enforce via custom `tracing` layer that scrubs patterns:

```rust
// In tracing subscriber setup:
// Redact anything that looks like a Bearer token or looks like a base64 blob > 40 chars
// This is defense-in-depth; code should never construct log messages with tokens at all.
```

## Dependency Security

- Run `cargo audit` in CI to check for known vulnerabilities in dependencies.
- Pin exact versions in `Cargo.lock` and commit it.
- Review `cargo deny` output for license compliance and duplicate crates.
- Minimize dependencies — prefer `std` over crates where practical.

## Incident Response

If a token leak is detected or suspected:

1. Call `RemoveAccount` via D-Bus or CLI — this deletes the keyring entry
2. Revoke the token at `https://myaccount.google.com/permissions`
3. Re-authenticate
4. Check logs for evidence of exfiltration

## Security Checklist (Pre-Release)

- [ ] `cargo audit` passes with no high/critical findings
- [ ] `cargo clippy` has no `unsafe` outside `kio-worker/`
- [ ] No tokens in any log file at level INFO or above
- [ ] Path traversal test cases pass
- [ ] Symlink escape test cases pass
- [ ] TLS cert validation cannot be disabled via config
- [ ] Keyring integration tested on Fedora (libsecret) and KDE (KWallet)
- [ ] `cargo deny` passes license and duplicate checks
