//! Token bucket rate limiter: max N sync starts per second.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// Token bucket: refills at a fixed rate, allows at most `capacity` tokens,
/// each sync start consumes one token.
#[derive(Debug)]
pub struct TokenBucket {
    /// Max tokens (sync starts) allowed per second.
    capacity_per_sec: u32,
    /// Last refill time (Instant for monotonic time).
    last_refill: AtomicU64,
    /// Available tokens (stored as u64; we use 0..=capacity_per_sec).
    tokens: AtomicU64,
}

impl TokenBucket {
    /// Creates a rate limiter allowing at most `capacity_per_sec` sync starts per second.
    pub fn new(capacity_per_sec: u32) -> Self {
        let now = Self::now_instant_u64();
        Self {
            capacity_per_sec,
            last_refill: AtomicU64::new(now),
            tokens: AtomicU64::new(capacity_per_sec as u64),
        }
    }

    /// Returns true if a token was consumed (caller may proceed with sync).
    /// Returns false if no token available (caller should wait and retry).
    pub fn try_acquire(&self) -> bool {
        self.refill();
        let mut t = self.tokens.load(Ordering::Acquire);
        loop {
            if t == 0 {
                return false;
            }
            match self
                .tokens
                .compare_exchange_weak(t, t - 1, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => return true,
                Err(actual) => t = actual,
            }
        }
    }

    fn refill(&self) {
        let now = Self::now_instant_u64();
        let last = self.last_refill.load(Ordering::Acquire);
        let elapsed_nanos = now.saturating_sub(last);
        let elapsed_secs = elapsed_nanos as f64 / 1_000_000_000.0;
        if elapsed_secs >= 1.0
            && self
                .last_refill
                .compare_exchange(last, now, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
        {
            let to_add = (elapsed_secs * self.capacity_per_sec as f64)
                .min(self.capacity_per_sec as f64) as u64;
            if to_add > 0 {
                self.tokens.fetch_add(to_add, Ordering::Release);
            }
        }
        // Cap at capacity (in case we over-refilled)
        let _ = self
            .tokens
            .fetch_min(self.capacity_per_sec as u64, Ordering::Release);
    }

    /// Monotonic time in nanoseconds (for refill math). Uses Instant::now() elapsed since a fixed point.
    fn now_instant_u64() -> u64 {
        static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
        let start = START.get_or_init(Instant::now);
        start.elapsed().as_nanos() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn token_bucket_consumes_tokens() {
        let bucket = TokenBucket::new(2);
        assert!(bucket.try_acquire());
        assert!(bucket.try_acquire());
        assert!(!bucket.try_acquire());
    }

    #[test]
    fn token_bucket_refills_after_time() {
        let bucket = TokenBucket::new(1);
        assert!(bucket.try_acquire());
        assert!(!bucket.try_acquire());
        std::thread::sleep(Duration::from_secs(1));
        assert!(bucket.try_acquire());
    }
}
