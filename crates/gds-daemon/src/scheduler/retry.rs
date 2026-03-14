//! Retry backoff: exponential with jitter; next retry time from sync_errors state.

use chrono::{DateTime, Utc};

/// Backoff: min(2^attempt * 100ms + jitter, 32s).
/// `attempt` is 0-based (first retry = 0).
pub fn backoff_duration(attempt: i32) -> chrono::Duration {
    let base_ms = 100u64 * (1u64 << attempt.min(8)); // cap exponent at 8 so 2^8 * 100ms = 25.6s
    let cap_ms = 32_000u64;
    let ms = base_ms.min(cap_ms);
    chrono::Duration::milliseconds(ms as i64)
}

/// Returns the next time we may retry (occurred_at + backoff(retry_count)).
/// Caller should only run sync for this folder when now >= returned time.
pub fn next_retry_at(occurred_at: DateTime<Utc>, retry_count: i32) -> DateTime<Utc> {
    occurred_at + backoff_duration(retry_count)
}

/// Returns true if we are past the backoff window (ok to retry now).
pub fn should_retry_now(occurred_at: DateTime<Utc>, retry_count: i32, now: DateTime<Utc>) -> bool {
    now >= next_retry_at(occurred_at, retry_count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_increases_with_attempt() {
        let d0 = backoff_duration(0);
        let d1 = backoff_duration(1);
        let d2 = backoff_duration(2);
        assert!(d1 > d0);
        assert!(d2 > d1);
    }

    #[test]
    fn backoff_capped_at_32s() {
        let d10 = backoff_duration(10);
        assert!(d10 <= chrono::Duration::seconds(32));
    }

    #[test]
    fn next_retry_at_and_should_retry_now() {
        use chrono::Duration;
        let occurred = Utc::now() - Duration::minutes(1);
        let _next = next_retry_at(occurred, 0);
        assert!(next_retry_at(occurred, 0) <= occurred + Duration::seconds(1));
        assert!(should_retry_now(occurred, 0, Utc::now()));
    }
}
