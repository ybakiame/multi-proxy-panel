//! In-memory token-bucket rate limiter for API keys.
//!
//! Used as a stop-gap until a persistent backend (Redis) is available.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Clone)]
struct BucketState {
    tokens: u64,
    last_update: Instant,
}

#[derive(Clone, Default)]
pub struct RateLimiter {
    buckets: Arc<Mutex<HashMap<String, BucketState>>>,
}

/// Maximum age for a bucket before it's eligible for cleanup.
const BUCKET_MAX_AGE: Duration = Duration::from_secs(3600); // 1 hour

impl RateLimiter {
    /// Allow one request if the bucket has capacity.
    ///
    /// Window is fixed at one minute; `limit` is the number of requests allowed
    /// per minute.
    pub async fn check(&self, key: &str, limit: u64) -> bool {
        if limit == 0 {
            return true;
        }

        let now = Instant::now();
        let window = Duration::from_secs(60);

        let mut buckets = self.buckets.lock().unwrap();

        // Periodic cleanup: remove stale buckets on each check
        if buckets.len() > 1000 {
            buckets.retain(|_, bucket| now.duration_since(bucket.last_update) < BUCKET_MAX_AGE);
        }

        let bucket = buckets.entry(key.to_string()).or_insert_with(|| BucketState {
            tokens: limit,
            last_update: now,
        });

        let elapsed = now.duration_since(bucket.last_update);
        let replenish = (elapsed.as_secs_f64() / window.as_secs_f64() * limit as f64) as u64;
        bucket.tokens = (bucket.tokens + replenish).min(limit);
        bucket.last_update = now;

        if bucket.tokens > 0 {
            bucket.tokens -= 1;
            true
        } else {
            false
        }
    }

    /// Explicitly clear all rate limit state (e.g., on config reload).
    pub fn clear(&self) {
        let mut buckets = self.buckets.lock().unwrap();
        buckets.clear();
    }

    /// Return the number of active buckets (for diagnostics).
    pub fn bucket_count(&self) -> usize {
        self.buckets.lock().unwrap().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn basic_rate_limit() {
        let limiter = RateLimiter::default();
        assert!(limiter.check("test", 2).await);
        assert!(limiter.check("test", 2).await);
        assert!(!limiter.check("test", 2).await);
    }

    #[tokio::test]
    async fn keys_are_isolated() {
        let limiter = RateLimiter::default();
        assert!(limiter.check("a", 1).await);
        assert!(!limiter.check("a", 1).await);
        assert!(limiter.check("b", 1).await);
    }

    #[tokio::test]
    async fn zero_limit_allows_all() {
        let limiter = RateLimiter::default();
        for _ in 0..100 {
            assert!(limiter.check("unlimited", 0).await);
        }
    }

    #[tokio::test]
    async fn clear_resets_state() {
        let limiter = RateLimiter::default();
        limiter.check("key", 1).await;
        assert_eq!(limiter.bucket_count(), 1);
        limiter.clear();
        assert_eq!(limiter.bucket_count(), 0);
    }
}
