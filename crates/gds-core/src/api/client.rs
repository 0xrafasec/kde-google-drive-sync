//! Google Drive API v3 client (reqwest, base URL injectable for testing).

use std::time::Duration;

use reqwest::multipart;
use reqwest::Client;
use tracing::instrument;

use crate::api::backoff::{is_retryable_error, BackoffPolicy};
use crate::api::error::status_to_sync_error;
use crate::api::types::{
    AboutResponse, CreateFileMetadata, DriveListResponse, FileListResponse, UpdateFileMetadata,
};
use crate::model::{ChangeSet, Config, DriveFile, SyncError};

/// Default Drive API base URL.
pub const DEFAULT_BASE_URL: &str = "https://www.googleapis.com";

/// Minimal file fields for list/get (partial response).
pub const FILE_FIELDS: &str = "id,name,mimeType,md5Checksum,size,modifiedTime,parents,trashed";

/// Fields for changes.list.
pub const CHANGES_FIELDS: &str =
    "nextPageToken,newStartPageToken,changes(changeType,fileId,file(id,name,mimeType,md5Checksum,size,modifiedTime,parents,trashed),removed)";

/// Max size for simple upload (multipart): 5 MB.
pub const SIMPLE_UPLOAD_MAX_BYTES: u64 = 5 * 1024 * 1024;

/// Chunk size for resumable uploads (8 MB recommended).
pub const RESUMABLE_CHUNK_SIZE: usize = 8 * 1024 * 1024;

/// Drive API v3 client. Base URL is injectable for testing (e.g. wiremock).
#[derive(Clone)]
pub struct DriveClient {
    client: Client,
    base_url: String,
    request_timeout_secs: u64,
    upload_timeout_secs: u64,
    backoff: BackoffPolicy,
}

impl DriveClient {
    /// Builds a client with default base URL and config timeouts.
    pub fn new(config: &Config) -> Result<Self, SyncError> {
        Self::with_base_url(DEFAULT_BASE_URL.to_string(), config)
    }

    /// Builds a client with a custom base URL (for tests: wiremock server URL).
    pub fn with_base_url(base_url: String, config: &Config) -> Result<Self, SyncError> {
        let base = base_url.trim_end_matches('/').to_string();
        let client = Client::builder().build().map_err(|e| SyncError::ApiError {
            code: 0,
            message: e.to_string(),
        })?;
        Ok(Self {
            client,
            base_url: base,
            request_timeout_secs: config.sync.request_timeout_secs,
            upload_timeout_secs: config.sync.upload_timeout_secs,
            backoff: BackoffPolicy::default(),
        })
    }

    fn drive_url(&self, path: &str) -> String {
        format!("{}/drive/v3{}", self.base_url, path)
    }

    fn upload_url(&self, path: &str) -> String {
        format!("{}/upload/drive/v3{}", self.base_url, path)
    }

    fn request_timeout(&self) -> Duration {
        Duration::from_secs(self.request_timeout_secs)
    }

    fn upload_timeout(&self) -> Duration {
        Duration::from_secs(self.upload_timeout_secs)
    }

    /// Executes a request that returns JSON, with retry on 429/5xx.
    async fn execute_json<T: serde::de::DeserializeOwned>(
        &self,
        method: reqwest::Method,
        url: &str,
        access_token: &str,
        timeout: Duration,
        body: Option<serde_json::Value>,
    ) -> Result<T, SyncError> {
        self.execute_with_retry(|| {
            tracing::trace!(method = %method, url = %url, "request");
            let url = url.to_string();
            let access_token = access_token.to_string();
            let timeout = timeout;
            let body = body.clone();
            let client = self.client.clone();
            let method = method.clone();
            async move {
                let mut req = client
                    .request(method, &url)
                    .header("Authorization", format!("Bearer {}", access_token))
                    .timeout(timeout);
                if let Some(b) = body {
                    req = req.json(&b);
                }
                let res = req.send().await.map_err(|e| SyncError::ApiError {
                    code: 0,
                    message: e.to_string(),
                })?;
                Self::check_response(res).await
            }
        })
        .await
    }

    async fn check_response<T: serde::de::DeserializeOwned>(
        res: reqwest::Response,
    ) -> Result<T, SyncError> {
        let status = res.status();
        if status.is_success() {
            let body = res.bytes().await.map_err(|e| SyncError::ApiError {
                code: 0,
                message: e.to_string(),
            })?;
            tracing::trace!(status = %status, body_len = body.len(), "response");
            serde_json::from_slice(&body).map_err(|e| SyncError::ApiError {
                code: 0,
                message: e.to_string(),
            })
        } else {
            let headers = res.headers().clone();
            let body = res.text().await.unwrap_or_default();
            Err(status_to_sync_error(status, &headers, &body))
        }
    }

    async fn execute_with_retry<F, Fut, T>(&self, operation: F) -> Result<T, SyncError>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, SyncError>>,
    {
        let mut attempt = 0u32;
        loop {
            match operation().await {
                Ok(v) => return Ok(v),
                Err(e) if is_retryable_error(&e) && attempt < self.backoff.max_attempts => {
                    attempt += 1;
                    let delay = self.backoff.delay_for_attempt(attempt - 1);
                    tracing::warn!(
                        attempt = attempt,
                        "retryable error, backing off {:?}: {}",
                        delay,
                        e
                    );
                    tokio::time::sleep(delay).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// GET request that returns bytes (for alt=media or export). Streams to the given writer.
    async fn execute_stream_to_writer<W>(
        &self,
        url: &str,
        access_token: &str,
        timeout: Duration,
        writer: &mut W,
    ) -> Result<(), SyncError>
    where
        W: tokio::io::AsyncWriteExt + Unpin,
    {
        let res = self
            .client
            .get(url)
            .header("Authorization", format!("Bearer {}", access_token))
            .timeout(timeout)
            .send()
            .await
            .map_err(|e| SyncError::ApiError {
                code: 0,
                message: e.to_string(),
            })?;
        if !res.status().is_success() {
            let status = res.status();
            let headers = res.headers().clone();
            let body = res.text().await.unwrap_or_default();
            return Err(status_to_sync_error(status, &headers, &body));
        }
        let bytes = res.bytes().await.map_err(|e| SyncError::ApiError {
            code: 0,
            message: e.to_string(),
        })?;
        writer
            .write_all(&bytes)
            .await
            .map_err(|e| SyncError::IoError {
                path: String::new(),
                source: e,
            })?;
        writer.flush().await.map_err(|e| SyncError::IoError {
            path: String::new(),
            source: e,
        })?;
        Ok(())
    }

    // ---------- files ----------

    /// `files.list` — paginated list with optional q, orderBy, pageSize.
    #[instrument(skip(self, access_token), level = "debug")]
    pub async fn files_list(
        &self,
        access_token: &str,
        q: Option<&str>,
        page_token: Option<&str>,
        page_size: Option<u32>,
        order_by: Option<&str>,
        fields: &str,
    ) -> Result<FileListResponse, SyncError> {
        let page_size_str = page_size.map(|s| s.to_string());
        let mut query: Vec<(&str, &str)> = vec![("fields", fields)];
        if let Some(q) = q {
            query.push(("q", q));
        }
        if let Some(t) = page_token {
            query.push(("pageToken", t));
        }
        if let Some(ref s) = page_size_str {
            query.push(("pageSize", s));
        }
        if let Some(o) = order_by {
            query.push(("orderBy", o));
        }
        let url =
            reqwest::Url::parse_with_params(&self.drive_url("/files"), &query).map_err(|e| {
                SyncError::ApiError {
                    code: 0,
                    message: e.to_string(),
                }
            })?;
        self.execute_json(
            reqwest::Method::GET,
            url.as_str(),
            access_token,
            self.request_timeout(),
            None,
        )
        .await
    }

    /// `files.get` — metadata only with fields selection.
    #[instrument(skip(self, access_token), level = "debug")]
    pub async fn files_get(
        &self,
        access_token: &str,
        file_id: &str,
        fields: &str,
    ) -> Result<DriveFile, SyncError> {
        let url = reqwest::Url::parse_with_params(
            &self.drive_url(&format!("/files/{}", file_id)),
            &[("fields", fields)],
        )
        .map_err(|e| SyncError::ApiError {
            code: 0,
            message: e.to_string(),
        })?;
        self.execute_json(
            reqwest::Method::GET,
            url.as_str(),
            access_token,
            self.request_timeout(),
            None,
        )
        .await
    }

    /// `files.get?alt=media` — download file content to the given async writer.
    #[instrument(skip(self, access_token, writer), level = "debug")]
    pub async fn files_get_media<W>(
        &self,
        access_token: &str,
        file_id: &str,
        writer: &mut W,
    ) -> Result<(), SyncError>
    where
        W: tokio::io::AsyncWriteExt + Unpin,
    {
        let url = reqwest::Url::parse_with_params(
            &self.drive_url(&format!("/files/{}", file_id)),
            &[("alt", "media")],
        )
        .map_err(|e| SyncError::ApiError {
            code: 0,
            message: e.to_string(),
        })?;
        self.execute_stream_to_writer(url.as_str(), access_token, self.upload_timeout(), writer)
            .await
    }

    /// `files.export` — export Google Workspace file to the given MIME type; stream to writer.
    #[instrument(skip(self, access_token, writer), level = "debug")]
    pub async fn files_export<W>(
        &self,
        access_token: &str,
        file_id: &str,
        export_mime_type: &str,
        writer: &mut W,
    ) -> Result<(), SyncError>
    where
        W: tokio::io::AsyncWriteExt + Unpin,
    {
        let url = reqwest::Url::parse_with_params(
            &self.drive_url(&format!("/files/{}/export", file_id)),
            &[("mimeType", export_mime_type)],
        )
        .map_err(|e| SyncError::ApiError {
            code: 0,
            message: e.to_string(),
        })?;
        self.execute_stream_to_writer(url.as_str(), access_token, self.upload_timeout(), writer)
            .await
    }

    /// `files.create` — simple upload (multipart) for files ≤5 MB.
    #[instrument(skip(self, access_token, content), level = "debug")]
    pub async fn files_create_simple(
        &self,
        access_token: &str,
        metadata: &CreateFileMetadata,
        content: &[u8],
        mime_type: &str,
    ) -> Result<DriveFile, SyncError> {
        if content.len() as u64 > SIMPLE_UPLOAD_MAX_BYTES {
            return Err(SyncError::ApiError {
                code: 0,
                message: "file too large for simple upload; use resumable".to_string(),
            });
        }
        let meta_json = serde_json::to_string(metadata).map_err(|e| SyncError::ApiError {
            code: 0,
            message: e.to_string(),
        })?;
        let meta_part = multipart::Part::text(meta_json)
            .mime_str("application/json")
            .map_err(|e| SyncError::ApiError {
                code: 0,
                message: e.to_string(),
            })?;
        let file_part = multipart::Part::bytes(content.to_vec())
            .mime_str(mime_type)
            .map_err(|e| SyncError::ApiError {
                code: 0,
                message: e.to_string(),
            })?;
        let form = multipart::Form::new()
            .part("metadata", meta_part)
            .part("file", file_part);
        let url = reqwest::Url::parse_with_params(
            &self.upload_url("/files"),
            &[("uploadType", "multipart"), ("fields", FILE_FIELDS)],
        )
        .map_err(|e| SyncError::ApiError {
            code: 0,
            message: e.to_string(),
        })?;
        let res = self
            .client
            .post(url.as_str())
            .header("Authorization", format!("Bearer {}", access_token))
            .timeout(self.upload_timeout())
            .multipart(form)
            .send()
            .await
            .map_err(|e| SyncError::ApiError {
                code: 0,
                message: e.to_string(),
            })?;
        if !res.status().is_success() {
            let status = res.status();
            let headers = res.headers().clone();
            let body = res.text().await.unwrap_or_default();
            return Err(status_to_sync_error(status, &headers, &body));
        }
        let file: DriveFile = res.json().await.map_err(|e| SyncError::ApiError {
            code: 0,
            message: e.to_string(),
        })?;
        Ok(file)
    }

    /// `files.create` — resumable upload: initiate then upload in chunks. Supports resume on 308.
    #[instrument(skip(self, access_token, reader, progress), level = "debug")]
    pub async fn files_create_resumable<R>(
        &self,
        access_token: &str,
        metadata: &CreateFileMetadata,
        total_size: u64,
        mime_type: &str,
        reader: &mut R,
        progress: Option<&mut (dyn FnMut(u64, u64) + Send)>,
    ) -> Result<DriveFile, SyncError>
    where
        R: tokio::io::AsyncReadExt + Unpin + Send,
    {
        let url_init = reqwest::Url::parse_with_params(
            &self.upload_url("/files"),
            &[("uploadType", "resumable")],
        )
        .map_err(|e| SyncError::ApiError {
            code: 0,
            message: e.to_string(),
        })?;
        let res = self
            .client
            .post(url_init.as_str())
            .header("Authorization", format!("Bearer {}", access_token))
            .header("X-Upload-Content-Type", mime_type)
            .header("X-Upload-Content-Length", total_size.to_string())
            .json(metadata)
            .timeout(self.request_timeout())
            .send()
            .await
            .map_err(|e| SyncError::ApiError {
                code: 0,
                message: e.to_string(),
            })?;
        if !res.status().is_success() {
            let status = res.status();
            let headers = res.headers().clone();
            let body = res.text().await.unwrap_or_default();
            return Err(status_to_sync_error(status, &headers, &body));
        }
        let upload_uri = res
            .headers()
            .get("Location")
            .and_then(|v| v.to_str().ok())
            .map(String::from)
            .ok_or_else(|| SyncError::ApiError {
                code: 0,
                message: "resumable upload: missing Location header".to_string(),
            })?;
        self.upload_chunks_resumable(&upload_uri, access_token, total_size, 0, reader, progress)
            .await
    }

    /// Resumable upload: send chunks to upload_uri until 200/201. Pass start_offset for resume.
    async fn upload_chunks_resumable<R>(
        &self,
        upload_uri: &str,
        access_token: &str,
        total_size: u64,
        start_offset: u64,
        reader: &mut R,
        mut progress: Option<&mut (dyn FnMut(u64, u64) + Send)>,
    ) -> Result<DriveFile, SyncError>
    where
        R: tokio::io::AsyncReadExt + Unpin + Send,
    {
        let mut offset = start_offset;
        if start_offset > 0 {
            let mut buf = vec![0u8; RESUMABLE_CHUNK_SIZE];
            let mut skip = start_offset;
            while skip > 0 {
                let to_read = (skip as usize).min(buf.len());
                let n = reader
                    .read(&mut buf[..to_read])
                    .await
                    .map_err(|e| SyncError::IoError {
                        path: String::new(),
                        source: e,
                    })?;
                if n == 0 {
                    return Err(SyncError::ApiError {
                        code: 0,
                        message: "resumable upload: reader ended before start_offset".to_string(),
                    });
                }
                skip -= n as u64;
            }
        }
        let mut buf = vec![0u8; RESUMABLE_CHUNK_SIZE];
        loop {
            let n = reader
                .read(&mut buf)
                .await
                .map_err(|e| SyncError::IoError {
                    path: String::new(),
                    source: e,
                })?;
            if n == 0 {
                if offset < total_size {
                    return Err(SyncError::ApiError {
                        code: 0,
                        message: "resumable upload: stream ended before total_size".to_string(),
                    });
                }
                break;
            }
            let chunk = &buf[..n];
            let end = offset + n as u64 - 1;
            let content_range = format!("bytes {}-{}/{}", offset, end, total_size);
            let res = self
                .client
                .put(upload_uri)
                .header("Authorization", format!("Bearer {}", access_token))
                .header("Content-Range", content_range)
                .body(chunk.to_vec())
                .timeout(self.upload_timeout())
                .send()
                .await
                .map_err(|e| SyncError::ApiError {
                    code: 0,
                    message: e.to_string(),
                })?;
            match res.status().as_u16() {
                200 | 201 => {
                    let file: DriveFile = res.json().await.map_err(|e| SyncError::ApiError {
                        code: 0,
                        message: e.to_string(),
                    })?;
                    return Ok(file);
                }
                308 => {
                    // Server received bytes 0..=offset+n-1; next byte to send is offset+n
                    offset += n as u64;
                    if let Some(ref mut cb) = progress {
                        cb(offset, total_size);
                    }
                }
                _ => {
                    let status = res.status();
                    let headers = res.headers().clone();
                    let body = res.text().await.unwrap_or_default();
                    return Err(status_to_sync_error(status, &headers, &body));
                }
            }
        }
        // Last request with Content-Range: bytes */* to commit if no more data
        let res = self
            .client
            .put(upload_uri)
            .header("Authorization", format!("Bearer {}", access_token))
            .header("Content-Range", format!("bytes */{}", total_size))
            .body(vec![])
            .timeout(self.request_timeout())
            .send()
            .await
            .map_err(|e| SyncError::ApiError {
                code: 0,
                message: e.to_string(),
            })?;
        let status = res.status();
        if status.as_u16() == 200 || status.as_u16() == 201 {
            let file: DriveFile = res.json().await.map_err(|e| SyncError::ApiError {
                code: 0,
                message: e.to_string(),
            })?;
            Ok(file)
        } else {
            let headers = res.headers().clone();
            let body = res.text().await.unwrap_or_default();
            Err(status_to_sync_error(status, &headers, &body))
        }
    }

    /// `files.update` — metadata only (rename, move, trash).
    #[instrument(skip(self, access_token), level = "debug")]
    pub async fn files_update_metadata(
        &self,
        access_token: &str,
        file_id: &str,
        metadata: &UpdateFileMetadata,
        fields: &str,
    ) -> Result<DriveFile, SyncError> {
        let url = reqwest::Url::parse_with_params(
            &self.drive_url(&format!("/files/{}", file_id)),
            &[("fields", fields)],
        )
        .map_err(|e| SyncError::ApiError {
            code: 0,
            message: e.to_string(),
        })?;
        self.execute_json(
            reqwest::Method::PATCH,
            url.as_str(),
            access_token,
            self.request_timeout(),
            Some(
                serde_json::to_value(metadata).map_err(|e| SyncError::ApiError {
                    code: 0,
                    message: e.to_string(),
                })?,
            ),
        )
        .await
    }

    /// `files.update` — content via simple (multipart) upload.
    #[instrument(skip(self, access_token, content), level = "debug")]
    pub async fn files_update_content_simple(
        &self,
        access_token: &str,
        file_id: &str,
        content: &[u8],
        mime_type: &str,
    ) -> Result<DriveFile, SyncError> {
        if content.len() as u64 > SIMPLE_UPLOAD_MAX_BYTES {
            return Err(SyncError::ApiError {
                code: 0,
                message: "file too large for simple update; use resumable".to_string(),
            });
        }
        let file_part = multipart::Part::bytes(content.to_vec())
            .mime_str(mime_type)
            .map_err(|e| SyncError::ApiError {
                code: 0,
                message: e.to_string(),
            })?;
        let form = multipart::Form::new().part("file", file_part);
        let url = reqwest::Url::parse_with_params(
            &self.upload_url(&format!("/files/{}", file_id)),
            &[("uploadType", "multipart"), ("fields", FILE_FIELDS)],
        )
        .map_err(|e| SyncError::ApiError {
            code: 0,
            message: e.to_string(),
        })?;
        let res = self
            .client
            .patch(url.as_str())
            .header("Authorization", format!("Bearer {}", access_token))
            .timeout(self.upload_timeout())
            .multipart(form)
            .send()
            .await
            .map_err(|e| SyncError::ApiError {
                code: 0,
                message: e.to_string(),
            })?;
        if !res.status().is_success() {
            let status = res.status();
            let headers = res.headers().clone();
            let body = res.text().await.unwrap_or_default();
            return Err(status_to_sync_error(status, &headers, &body));
        }
        res.json().await.map_err(|e| SyncError::ApiError {
            code: 0,
            message: e.to_string(),
        })
    }

    /// `files.update` — content via resumable upload; resume from offset on 308 after failure.
    #[instrument(skip(self, access_token, reader, progress), level = "debug")]
    pub async fn files_update_content_resumable<R>(
        &self,
        access_token: &str,
        file_id: &str,
        total_size: u64,
        mime_type: &str,
        start_offset: u64,
        reader: &mut R,
        progress: Option<&mut (dyn FnMut(u64, u64) + Send)>,
    ) -> Result<DriveFile, SyncError>
    where
        R: tokio::io::AsyncReadExt + Unpin + Send,
    {
        let url_init = reqwest::Url::parse_with_params(
            &self.upload_url(&format!("/files/{}", file_id)),
            &[("uploadType", "resumable")],
        )
        .map_err(|e| SyncError::ApiError {
            code: 0,
            message: e.to_string(),
        })?;
        let res = self
            .client
            .patch(url_init.as_str())
            .header("Authorization", format!("Bearer {}", access_token))
            .header("X-Upload-Content-Type", mime_type)
            .header("X-Upload-Content-Length", total_size.to_string())
            .timeout(self.request_timeout())
            .send()
            .await
            .map_err(|e| SyncError::ApiError {
                code: 0,
                message: e.to_string(),
            })?;
        if !res.status().is_success() {
            let status = res.status();
            let headers = res.headers().clone();
            let body = res.text().await.unwrap_or_default();
            return Err(status_to_sync_error(status, &headers, &body));
        }
        let upload_uri = res
            .headers()
            .get("Location")
            .and_then(|v| v.to_str().ok())
            .map(String::from)
            .ok_or_else(|| SyncError::ApiError {
                code: 0,
                message: "resumable update: missing Location header".to_string(),
            })?;
        self.upload_chunks_resumable(
            &upload_uri,
            access_token,
            total_size,
            start_offset,
            reader,
            progress,
        )
        .await
    }

    /// `files.delete` — permanent delete.
    #[instrument(skip(self, access_token), level = "debug")]
    pub async fn files_delete(&self, access_token: &str, file_id: &str) -> Result<(), SyncError> {
        let url = self.drive_url(&format!("/files/{}", file_id));
        let res = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", access_token))
            .timeout(self.request_timeout())
            .send()
            .await
            .map_err(|e| SyncError::ApiError {
                code: 0,
                message: e.to_string(),
            })?;
        let status = res.status();
        if status.as_u16() == 204 || status.is_success() {
            Ok(())
        } else {
            let headers = res.headers().clone();
            let body = res.text().await.unwrap_or_default();
            Err(status_to_sync_error(status, &headers, &body))
        }
    }

    /// `files.copy` — server-side copy.
    #[instrument(skip(self, access_token), level = "debug")]
    pub async fn files_copy(
        &self,
        access_token: &str,
        file_id: &str,
        name: Option<&str>,
        parents: Option<&[String]>,
    ) -> Result<DriveFile, SyncError> {
        let mut body = serde_json::Map::new();
        if let Some(n) = name {
            body.insert("name".to_string(), serde_json::Value::String(n.to_string()));
        }
        if let Some(p) = parents {
            body.insert(
                "parents".to_string(),
                serde_json::Value::Array(
                    p.iter()
                        .map(|s| serde_json::Value::String(s.clone()))
                        .collect(),
                ),
            );
        }
        let url = reqwest::Url::parse_with_params(
            &self.drive_url(&format!("/files/{}/copy", file_id)),
            &[("fields", FILE_FIELDS)],
        )
        .map_err(|e| SyncError::ApiError {
            code: 0,
            message: e.to_string(),
        })?;
        self.execute_json(
            reqwest::Method::POST,
            url.as_str(),
            access_token,
            self.request_timeout(),
            Some(serde_json::Value::Object(body)),
        )
        .await
    }

    // ---------- changes ----------

    /// `changes.getStartPageToken`
    #[instrument(skip(self, access_token), level = "debug")]
    pub async fn changes_get_start_page_token(
        &self,
        access_token: &str,
        drive_id: Option<&str>,
    ) -> Result<String, SyncError> {
        let mut query = vec![("fields", "startPageToken")];
        if let Some(d) = drive_id {
            query.push(("driveId", d));
        }
        let url =
            reqwest::Url::parse_with_params(&self.drive_url("/changes/startPageToken"), &query)
                .map_err(|e| SyncError::ApiError {
                    code: 0,
                    message: e.to_string(),
                })?;
        #[derive(serde::Deserialize)]
        struct StartToken {
            #[serde(rename = "startPageToken")]
            start_page_token: String,
        }
        let t: StartToken = self
            .execute_json(
                reqwest::Method::GET,
                url.as_str(),
                access_token,
                self.request_timeout(),
                None,
            )
            .await?;
        Ok(t.start_page_token)
    }

    /// `changes.list` — paginated, with pageToken and fields.
    #[instrument(skip(self, access_token), level = "debug")]
    pub async fn changes_list(
        &self,
        access_token: &str,
        page_token: &str,
        page_size: Option<u32>,
        fields: &str,
        include_items_from_all_drives: bool,
        supports_all_drives: bool,
    ) -> Result<ChangeSet, SyncError> {
        let page_size_str = page_size.map(|s| s.to_string());
        let mut query: Vec<(&str, &str)> = vec![
            ("pageToken", page_token),
            ("fields", fields),
            (
                "includeItemsFromAllDrives",
                if include_items_from_all_drives {
                    "true"
                } else {
                    "false"
                },
            ),
            (
                "supportsAllDrives",
                if supports_all_drives { "true" } else { "false" },
            ),
        ];
        if let Some(ref s) = page_size_str {
            query.push(("pageSize", s));
        }
        let url =
            reqwest::Url::parse_with_params(&self.drive_url("/changes"), &query).map_err(|e| {
                SyncError::ApiError {
                    code: 0,
                    message: e.to_string(),
                }
            })?;
        self.execute_json(
            reqwest::Method::GET,
            url.as_str(),
            access_token,
            self.request_timeout(),
            None,
        )
        .await
    }

    // ---------- about ----------

    /// `about.get` — quota and user info (partial response).
    #[instrument(skip(self, access_token), level = "debug")]
    pub async fn about_get(
        &self,
        access_token: &str,
        fields: &str,
    ) -> Result<AboutResponse, SyncError> {
        let url = reqwest::Url::parse_with_params(&self.drive_url("/about"), &[("fields", fields)])
            .map_err(|e| SyncError::ApiError {
                code: 0,
                message: e.to_string(),
            })?;
        self.execute_json(
            reqwest::Method::GET,
            url.as_str(),
            access_token,
            self.request_timeout(),
            None,
        )
        .await
    }

    // ---------- drives (stub) ----------

    /// `drive.list` — shared drives. Stub: returns empty list for now.
    #[instrument(skip(self, access_token), level = "debug")]
    pub async fn drive_list(
        &self,
        access_token: &str,
        _page_token: Option<&str>,
        _page_size: Option<u32>,
    ) -> Result<DriveListResponse, SyncError> {
        let _ = (access_token,);
        Ok(DriveListResponse {
            next_page_token: None,
            drives: vec![],
        })
    }
}
