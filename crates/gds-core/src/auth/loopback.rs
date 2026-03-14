//! Loopback HTTP server to receive the OAuth redirect (code + state).

use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::model::SyncError;

/// Default ports to try if the preferred port is in use.
const FALLBACK_PORTS: [u16; 5] = [8765, 8766, 8767, 8768, 8769];

/// Binds to 127.0.0.1 on the given port. If that port is in use, tries fallback ports.
/// Returns the actual port bound and the listener.
pub async fn bind_loopback(preferred_port: u16) -> Result<(TcpListener, u16), SyncError> {
    let ports = std::iter::once(preferred_port).chain(
        FALLBACK_PORTS
            .iter()
            .copied()
            .filter(move |&p| p != preferred_port),
    );
    for port in ports {
        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        if let Ok(listener) = TcpListener::bind(addr).await {
            return Ok((listener, port));
        }
    }
    Err(SyncError::AuthError {
        message: "Could not bind to any loopback port".to_string(),
    })
}

/// Parses query string into (code, state). Returns None if missing or malformed.
pub fn parse_redirect_query(query: &str) -> Option<(String, String)> {
    let mut code = None;
    let mut state = None;
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=')?;
        let v = urlencoding::decode(v).ok()?.to_string();
        match k {
            "code" => code = Some(v),
            "state" => state = Some(v),
            _ => {}
        }
    }
    Some((code?, state?))
}

/// Response body shown to the user after successful auth.
const SUCCESS_HTML: &str = r#"<!DOCTYPE html><html><head><title>Signed in</title></head><body><p>You are signed in. You can close this tab.</p></body></html>"#;
/// Response body shown on error (e.g. missing code).
const ERROR_HTML: &str = r#"<!DOCTYPE html><html><head><title>Error</title></head><body><p>Authorization failed. You can close this tab.</p></body></html>"#;

/// Waits for one GET request on the listener, parses the query string for code and state,
/// responds with a simple HTML page, and returns (code, state).
pub async fn wait_for_redirect(
    listener: TcpListener,
    timeout: Duration,
) -> Result<(String, String), SyncError> {
    let (mut stream, _) = tokio::time::timeout(timeout, async {
        listener
            .accept()
            .await
            .map_err(|e| SyncError::NetworkError(e))
    })
    .await
    .map_err(|_| SyncError::AuthError {
        message: "Timeout waiting for redirect".to_string(),
    })??;

    let mut buf = [0u8; 4096];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(SyncError::NetworkError)?;
    let request = String::from_utf8_lossy(&buf[..n]);

    let query = request
        .lines()
        .next()
        .and_then(|line| line.strip_prefix("GET "))
        .and_then(|rest| rest.split_whitespace().next())
        .and_then(|path| path.strip_prefix("/?"))
        .unwrap_or("");

    let (code, state) = parse_redirect_query(query).ok_or_else(|| SyncError::AuthError {
        message: "Redirect missing code or state".to_string(),
    })?;

    let (status, body) = if code.is_empty() {
        ("400 Bad Request", ERROR_HTML)
    } else {
        ("200 OK", SUCCESS_HTML)
    };

    let response = format!(
        "HTTP/1.1 {}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        body.len(),
        body
    );
    stream
        .write_all(response.as_bytes())
        .await
        .map_err(SyncError::NetworkError)?;
    stream.flush().await.map_err(SyncError::NetworkError)?;

    Ok((code, state))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_redirect_query() {
        let (code, state) = parse_redirect_query("code=auth_code_123&state=csrf_abc").unwrap();
        assert_eq!(code, "auth_code_123");
        assert_eq!(state, "csrf_abc");
        assert!(parse_redirect_query("").is_none());
        assert!(parse_redirect_query("code=only").is_none());
    }
}
