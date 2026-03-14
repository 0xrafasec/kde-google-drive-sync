//! Map HTTP responses and reqwest errors to [crate::model::SyncError].

use reqwest::StatusCode;

use crate::model::SyncError;

/// Parses `Retry-After` header (seconds or HTTP-date). Returns seconds.
pub fn retry_after_seconds(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    let v = headers.get("retry-after")?.to_str().ok()?;
    if let Ok(secs) = v.parse::<u64>() {
        return Some(secs);
    }
    // HTTP-date could be parsed here; for simplicity we use 60s fallback when numeric parse fails
    Some(60)
}

/// Maps an HTTP status to SyncError. Uses response headers for 429 Retry-After.
pub fn status_to_sync_error(
    status: StatusCode,
    headers: &reqwest::header::HeaderMap,
    body: &str,
) -> SyncError {
    match status.as_u16() {
        401 => SyncError::AuthExpired,
        429 => {
            let retry_after = retry_after_seconds(headers).unwrap_or(60);
            SyncError::QuotaExceeded { retry_after }
        }
        code => SyncError::ApiError {
            code,
            message: body.to_string(),
        },
    }
}

/// Returns true for status codes that should be retried with backoff (429, 500, 502, 503, 504).
#[allow(dead_code)] // used by callers that map HTTP to retry decisions
pub fn is_retryable_status(status: StatusCode) -> bool {
    matches!(status.as_u16(), 429 | 500 | 502 | 503 | 504)
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue};

    #[test]
    fn retry_after_numeric() {
        let mut h = HeaderMap::new();
        h.insert("retry-after", HeaderValue::from_static("120"));
        assert_eq!(retry_after_seconds(&h), Some(120));
    }

    #[test]
    fn retry_after_missing() {
        let h = HeaderMap::new();
        assert_eq!(retry_after_seconds(&h), None);
    }

    #[test]
    fn status_401_auth_expired() {
        let h = HeaderMap::new();
        let e = status_to_sync_error(StatusCode::UNAUTHORIZED, &h, "Unauthorized");
        assert!(matches!(e, SyncError::AuthExpired));
    }

    #[test]
    fn status_429_quota_with_retry_after() {
        let mut h = HeaderMap::new();
        h.insert("retry-after", HeaderValue::from_static("45"));
        let e = status_to_sync_error(StatusCode::TOO_MANY_REQUESTS, &h, "Quota");
        assert!(matches!(e, SyncError::QuotaExceeded { retry_after: 45 }));
    }

    #[test]
    fn status_429_quota_default_60() {
        let h = HeaderMap::new();
        let e = status_to_sync_error(StatusCode::TOO_MANY_REQUESTS, &h, "Quota");
        assert!(matches!(e, SyncError::QuotaExceeded { retry_after: 60 }));
    }

    #[test]
    fn is_retryable() {
        assert!(is_retryable_status(StatusCode::TOO_MANY_REQUESTS));
        assert!(is_retryable_status(StatusCode::INTERNAL_SERVER_ERROR));
        assert!(is_retryable_status(StatusCode::BAD_GATEWAY));
        assert!(is_retryable_status(StatusCode::SERVICE_UNAVAILABLE));
        assert!(is_retryable_status(StatusCode::GATEWAY_TIMEOUT));
        assert!(!is_retryable_status(StatusCode::NOT_FOUND));
        assert!(!is_retryable_status(StatusCode::UNAUTHORIZED));
    }
}
