//! Typed sync errors (library crate: thiserror).


use thiserror::Error;

/// Sync and API errors. All failure modes are explicit for proper handling.
#[derive(Error, Debug)]
pub enum SyncError {
    #[error("API quota exceeded: retry after {retry_after}s")]
    QuotaExceeded { retry_after: u64 },

    #[error("Conflict detected for file {path}")]
    Conflict { path: String },

    #[error("Path traversal or invalid path: {path}")]
    PathTraversal { path: String },

    #[error("Authentication expired or invalid")]
    AuthExpired,

    #[error("Network error: {0}")]
    NetworkError(#[from] std::io::Error),

    #[error("I/O error at {path}: {source}")]
    IoError {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("Database error: {0}")]
    DatabaseError(#[from] Box<dyn std::error::Error + Send + Sync>),

    #[error("Drive API error: {code} — {message}")]
    ApiError { code: u16, message: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_error_display() {
        let e = SyncError::QuotaExceeded { retry_after: 60 };
        assert!(e.to_string().contains("60"));
        let e2 = SyncError::Conflict { path: "/a/b".to_string() };
        assert!(e2.to_string().contains("Conflict"));
        let e3 = SyncError::PathTraversal { path: "/etc/passwd".to_string() };
        assert!(e3.to_string().contains("Path traversal"));
        let e4 = SyncError::ApiError { code: 404, message: "Not Found".to_string() };
        assert!(e4.to_string().contains("404"));
    }
}
