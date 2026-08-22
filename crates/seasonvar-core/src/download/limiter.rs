//! A process-wide token bucket: every segment of every Job draws from the same budget.
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

/// Process-wide byte-rate limiter shared by every segment. `0` = unlimited.
pub struct RateLimiter {
    bytes_per_sec: AtomicU64,
    bucket: Mutex<Bucket>,
}

struct Bucket {
    tokens: f64,
    last: Instant,
}

impl RateLimiter {
    pub fn new(bytes_per_sec: u64) -> Self {
        RateLimiter {
            bytes_per_sec: AtomicU64::new(bytes_per_sec),
            bucket: Mutex::new(Bucket {
                tokens: 0.0,
                last: Instant::now(),
            }),
        }
    }

    /// Change the rate on the fly; in-flight `throttle` calls pick it up on their next chunk.
    pub fn set_rate(&self, bytes_per_sec: u64) {
        self.bytes_per_sec.store(bytes_per_sec, Ordering::Relaxed);
    }

    /// Wait until `n` bytes may pass. Burst capacity is one second of traffic.
    ///
    /// The bucket may go negative: a caller that overdraws it books the debt and sleeps until
    /// the refill covers it, so concurrent segments queue behind each other instead of each
    /// sleeping for its own deficit in parallel (which would multiply the rate by the number
    /// of segments).
    pub async fn throttle(&self, n: usize) {
        let rate = self.bytes_per_sec.load(Ordering::Relaxed);
        if rate == 0 {
            return;
        }
        let rate = rate as f64;
        let wait = {
            let mut b = self.bucket.lock().await;
            let now = Instant::now();
            b.tokens = (b.tokens + now.duration_since(b.last).as_secs_f64() * rate).min(rate);
            b.last = now;
            b.tokens -= n as f64;
            if b.tokens >= 0.0 {
                None
            } else {
                Some(Duration::from_secs_f64(-b.tokens / rate))
            }
        };
        if let Some(d) = wait {
            tokio::time::sleep(d).await;
        }
    }
}
