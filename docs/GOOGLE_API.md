# Google Drive API Integration Guide

## Setup

### 1. Google Cloud Project

1. Go to [console.cloud.google.com](https://console.cloud.google.com)
2. Create a new project: `google-drive-sync-kde`
3. Enable: **Google Drive API v3**
4. Create credentials: **OAuth 2.0 Client ID** → **Desktop application**
5. Download `client_secret.json`
6. **NEVER commit this file** — add to `.gitignore`

### 2. Config Structure

Store non-secret config (client_id, not client_secret) in:
`~/.config/gds/config.toml`

```toml
[oauth]
client_id = "123456789-abc.apps.googleusercontent.com"
# client_secret is in keyring, NOT here
redirect_port = 8765   # localhost redirect port for auth flow

[sync]
poll_interval_secs = 30
max_concurrent_uploads = 2
max_concurrent_downloads = 4
conflict_suffix_format = ".conflict-%Y%m%d-%H%M%S"

[ui]
show_notifications = true
notification_timeout_ms = 5000
```

Store `client_secret` in keyring under key `gds-client-secret`.

## Drive API v3 Key Endpoints

### Files

| Operation | Endpoint | Notes |
|---|---|---|
| List files | `GET /drive/v3/files` | Use `q` for filtering |
| Get metadata | `GET /drive/v3/files/{fileId}` | Always use `fields` |
| Download | `GET /drive/v3/files/{fileId}?alt=media` | |
| Create | `POST /upload/drive/v3/files?uploadType=resumable` | |
| Update content | `PATCH /upload/drive/v3/files/{fileId}?uploadType=resumable` | |
| Update metadata | `PATCH /drive/v3/files/{fileId}` | |
| Delete | `DELETE /drive/v3/files/{fileId}` | |
| Copy | `POST /drive/v3/files/{fileId}/copy` | Server-side |

### Changes (Incremental Sync)

| Operation | Endpoint | Notes |
|---|---|---|
| Get start token | `GET /drive/v3/changes/startPageToken` | Once, on first sync |
| List changes | `GET /drive/v3/changes` | Use `pageToken` |
| Watch (push) | `POST /drive/v3/changes/watch` | Optional, reduces polling |

## Minimal Required Fields

Always request only the fields you need. This reduces quota usage and latency.

```rust
// Good — partial response
const FILE_FIELDS: &str = "id,name,mimeType,md5Checksum,size,modifiedTime,parents,trashed";
const CHANGES_FIELDS: &str = "nextPageToken,newStartPageToken,changes(changeType,fileId,file(id,name,mimeType,md5Checksum,size,modifiedTime,parents,trashed))";

// Bad — fetches everything including thumbnails, permissions, etc.
// fields = "*"
```

## Sync Algorithm (Changes API)

```rust
pub async fn sync_changes(
    client: &DriveClient,
    db: &Database,
    folder: &SyncFolder,
) -> Result<()> {
    let token = db.get_page_token(folder.id).await?;

    let mut page_token = token.unwrap_or_else(|| {
        // First run: get start token
        client.get_start_page_token().await
    });

    loop {
        let response = client.list_changes(ChangesRequest {
            page_token: &page_token,
            fields: CHANGES_FIELDS,
            spaces: "drive",
            include_items_from_all_drives: false,
            supports_all_drives: false,
        }).await?;

        for change in response.changes {
            process_change(change, folder, db).await?;
        }

        // Store progress after each page — survive crashes
        if let Some(next) = response.next_page_token {
            db.set_page_token(folder.id, &next).await?;
            page_token = next;
        } else {
            // Last page
            if let Some(new_start) = response.new_start_page_token {
                db.set_page_token(folder.id, &new_start).await?;
            }
            break;
        }
    }
    Ok(())
}
```

## Upload Strategy

### Simple Upload (≤5 MB)

```rust
async fn simple_upload(
    client: &reqwest::Client,
    token: &str,
    path: &Path,
    metadata: &DriveFileMetadata,
) -> Result<DriveFile> {
    let content = tokio::fs::read(path).await?;
    let mime = mime_guess::from_path(path).first_or_octet_stream();

    let response = client
        .post("https://www.googleapis.com/upload/drive/v3/files")
        .query(&[("uploadType", "multipart"), ("fields", FILE_FIELDS)])
        .bearer_auth(token)  // token.expose_secret() if using secrecy
        .multipart(
            reqwest::multipart::Form::new()
                .part("metadata", /* JSON metadata */)
                .part("file", reqwest::multipart::Part::bytes(content).mime_str(&mime.to_string())?)
        )
        .timeout(Duration::from_secs(30))
        .send()
        .await?
        .error_for_status()?
        .json::<DriveFile>()
        .await?;

    Ok(response)
}
```

### Resumable Upload (>5 MB)

```rust
async fn resumable_upload(
    client: &reqwest::Client,
    token: &str,
    path: &Path,
    metadata: &DriveFileMetadata,
) -> Result<DriveFile> {
    // Step 1: Initiate — get upload URI
    let init_response = client
        .post("https://www.googleapis.com/upload/drive/v3/files")
        .query(&[("uploadType", "resumable")])
        .bearer_auth(token)
        .header("X-Upload-Content-Type", mime_guess::from_path(path).first_or_octet_stream().to_string())
        .header("X-Upload-Content-Length", file_size)
        .json(metadata)
        .timeout(Duration::from_secs(30))
        .send()
        .await?;

    let upload_uri = init_response
        .headers()
        .get("Location")
        .ok_or(ApiError::MissingLocationHeader)?
        .to_str()?
        .to_string();

    // Step 2: Upload in chunks (8MB chunks recommended)
    const CHUNK_SIZE: usize = 8 * 1024 * 1024;
    let mut offset = 0u64;

    loop {
        let chunk = read_chunk(path, offset, CHUNK_SIZE).await?;
        let chunk_len = chunk.len() as u64;
        let total_size = file_size;
        let content_range = format!("bytes {offset}-{}/{total_size}", offset + chunk_len - 1);

        let resp = client
            .put(&upload_uri)
            .header("Content-Range", content_range)
            .body(chunk)
            .timeout(Duration::from_secs(300)) // 5 min for large chunks
            .send()
            .await?;

        match resp.status().as_u16() {
            200 | 201 => return Ok(resp.json::<DriveFile>().await?),
            308 => {
                // Resume Incomplete — get next expected offset
                offset = parse_range_header(resp.headers())? + 1;
            }
            _ => return Err(ApiError::UploadFailed(resp.status())),
        }
    }
}
```

## Rate Limiting & Backoff

### Quotas (as of 2024)

| Quota | Limit |
|---|---|
| Queries per second per user | 10 |
| Daily uploads | 750 GB |
| Files created per day | Unlimited |

### Backoff Implementation

```rust
pub struct BackoffPolicy {
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub max_attempts: u32,
}

impl Default for BackoffPolicy {
    fn default() -> Self {
        Self {
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(32),
            max_attempts: 8,
        }
    }
}

pub async fn with_backoff<T, F, Fut>(
    policy: &BackoffPolicy,
    operation: F,
) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut attempt = 0u32;
    loop {
        match operation().await {
            Ok(v) => return Ok(v),
            Err(e) if is_retryable(&e) && attempt < policy.max_attempts => {
                attempt += 1;
                let base = policy.base_delay * 2u32.pow(attempt);
                let jitter = Duration::from_millis(rand::random::<u64>() % 100);
                let delay = (base + jitter).min(policy.max_delay);
                tracing::warn!("Retryable error (attempt {attempt}), backing off {delay:?}: {e}");
                tokio::time::sleep(delay).await;
            }
            Err(e) => return Err(e),
        }
    }
}

fn is_retryable(err: &anyhow::Error) -> bool {
    if let Some(api_err) = err.downcast_ref::<ApiError>() {
        matches!(api_err, ApiError::RateLimited | ApiError::ServerError(_))
    } else {
        false
    }
}
```

## Google Workspace File Types

Google Docs, Sheets, Slides etc. cannot be downloaded as-is. They must be
exported to a compatible format.

| Google MIME Type | Export Format | Extension |
|---|---|---|
| `application/vnd.google-apps.document` | `application/vnd.openxmlformats-officedocument.wordprocessingml.document` | `.docx` |
| `application/vnd.google-apps.spreadsheet` | `application/vnd.openxmlformats-officedocument.spreadsheetml.sheet` | `.xlsx` |
| `application/vnd.google-apps.presentation` | `application/vnd.openxmlformats-officedocument.presentationml.presentation` | `.pptx` |
| `application/vnd.google-apps.drawing` | `image/svg+xml` | `.svg` |
| `application/vnd.google-apps.script` | `application/vnd.google-apps.script+json` | `.json` |

Export endpoint: `GET /drive/v3/files/{fileId}/export?mimeType={exportMimeType}`

**Note**: Google Workspace files cannot be uploaded back — they're read-only in
the sync. Show them with a `.gdoc`/`.gsheet` etc. shortcut stub file (like the
Windows client does) that opens the browser on click.

## Webhook Push Notifications (Optional, Post-MVP)

Instead of polling every 30s, register for push notifications:

```rust
// POST /drive/v3/changes/watch
let channel = WatchRequest {
    id: Uuid::new_v4().to_string(),
    type_: "web_hook",
    address: "https://your-server.example.com/drive-webhook",
    expiration: (now + Duration::from_secs(7 * 24 * 3600)).timestamp_millis(),
};
```

This requires a public HTTPS endpoint. For a desktop app, this is complex.
Consider using a relay service or falling back to polling. **MVP uses polling.**
