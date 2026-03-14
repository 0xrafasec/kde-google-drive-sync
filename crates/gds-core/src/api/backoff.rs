//! Exponential backoff with jitter for retryable Drive API errors.

use std::time::Duration;

use crate::model::SyncError;

/// Backoff policy: base_delay * 2^attempt + jitter, capped at max_delay.
#[derive(Clone, Debug)]
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

impl BackoffPolicy {
    /// Delay for the given 0-based attempt. Includes jitter (0–100 ms).
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let base = self.base_delay.saturating_mul(2u32.saturating_pow(attempt));
        let jitter_ms = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            % 101) as u64;
        let jitter = Duration::from_millis(jitter_ms);
        (base + jitter).min(self.max_delay)
    }
}

/// Returns true if the error is retryable (429 or 5xx).
pub fn is_retryable_error(err: &SyncError) -> bool {
    match err {
        SyncError::QuotaExceeded { .. } => true,
        SyncError::ApiError { code, .. } => matches!(code, 429 | 500 | 502 | 503 | 504),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_delay_increases() {
        let p = BackoffPolicy::default();
        let d0 = p.delay_for_attempt(0);
        let d1 = p.delay_for_attempt(1);
        let d2 = p.delay_for_attempt(2);
        assert!(d0 <= Duration::from_millis(200));
        assert!(d1 >= Duration::from_millis(100));
        assert!(d2 >= d1);
    }

    #[test]
    fn backoff_capped() {
        let p = BackoffPolicy::default();
        let d = p.delay_for_attempt(20);
        assert!(d <= p.max_delay);
    }

    #[test]
    fn quota_exceeded_retryable() {
        assert!(is_retryable_error(&SyncError::QuotaExceeded {
            retry_after: 60
        }));
    }

    #[test]
    fn api_error_429_retryable() {
        assert!(is_retryable_error(&SyncError::ApiError {
            code: 429,
            message: String::new()
        }));
    }

    #[test]
    fn api_error_404_not_retryable() {
        assert!(!is_retryable_error(&SyncError::ApiError {
            code: 404,
            message: String::new()
        }));
    }

    #[test]
    fn auth_expired_not_retryable() {
        assert!(!is_retryable_error(&SyncError::AuthExpired));
    }
}
