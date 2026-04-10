//! Rate limiter guardrail — limits the rate of calls.

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use tokio::sync::Mutex;

use lortex_core::guardrail::{Guardrail, GuardrailResult};
use lortex_core::message::Message;

/// A guardrail that enforces rate limits on agent execution.
pub struct RateLimiter {
    /// Maximum calls per minute.
    max_per_minute: u32,

    /// Track call timestamps.
    calls: Arc<Mutex<Vec<Instant>>>,
}

impl RateLimiter {
    /// Create a new rate limiter.
    pub fn new(max_per_minute: u32) -> Self {
        Self {
            max_per_minute,
            calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn check_rate(&self) -> GuardrailResult {
        let mut calls = self.calls.lock().await;
        let now = Instant::now();
        let one_minute_ago = now - std::time::Duration::from_secs(60);

        // Remove calls older than 1 minute
        calls.retain(|&t| t > one_minute_ago);

        if calls.len() >= self.max_per_minute as usize {
            return GuardrailResult::Block {
                message: format!(
                    "Rate limit exceeded: {} calls in the last minute (max: {})",
                    calls.len(),
                    self.max_per_minute
                ),
            };
        }

        calls.push(now);

        if calls.len() as f64 > self.max_per_minute as f64 * 0.8 {
            GuardrailResult::Warn {
                message: format!(
                    "Rate limit warning: {} / {} calls in the last minute (80%+)",
                    calls.len(),
                    self.max_per_minute
                ),
            }
        } else {
            GuardrailResult::Pass
        }
    }
}

#[async_trait]
impl Guardrail for RateLimiter {
    fn name(&self) -> &str {
        "rate_limiter"
    }

    async fn check_input(&self, _messages: &[Message]) -> GuardrailResult {
        self.check_rate().await
    }

    async fn check_output(&self, _output: &Message) -> GuardrailResult {
        GuardrailResult::Pass
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lortex_core::guardrail::Guardrail;
    use lortex_core::message::Message;

    #[test]
    fn name_returns_rate_limiter() {
        let limiter = RateLimiter::new(10);
        assert_eq!(limiter.name(), "rate_limiter");
    }

    #[tokio::test]
    async fn passes_under_limit() {
        let limiter = RateLimiter::new(10);
        let result = limiter.check_input(&[Message::user("hi")]).await;
        assert!(result.is_pass());
    }

    #[tokio::test]
    async fn warns_at_80_percent() {
        let limiter = RateLimiter::new(10);
        // Make 8 calls to reach 80% threshold
        for _ in 0..8 {
            limiter.check_input(&[Message::user("hi")]).await;
        }
        // 9th call should warn (9/10 = 90%)
        let result = limiter.check_input(&[Message::user("hi")]).await;
        assert!(result.is_warn());
    }

    #[tokio::test]
    async fn blocks_at_limit() {
        let limiter = RateLimiter::new(5);
        // Exhaust the limit
        for _ in 0..5 {
            limiter.check_input(&[Message::user("hi")]).await;
        }
        // Next call should be blocked
        let result = limiter.check_input(&[Message::user("hi")]).await;
        assert!(result.is_block());
    }

    #[tokio::test]
    async fn check_output_always_passes() {
        let limiter = RateLimiter::new(1);
        // Even after exhausting input limit
        for _ in 0..5 {
            limiter.check_input(&[Message::user("hi")]).await;
        }
        let result = limiter
            .check_output(&Message::assistant("response"))
            .await;
        assert!(result.is_pass());
    }

    #[tokio::test]
    async fn limit_of_one() {
        let limiter = RateLimiter::new(1);
        // First call passes (1/1 = 100%, but it's the first so it warns at >80%)
        let r1 = limiter.check_input(&[Message::user("hi")]).await;
        assert!(r1.is_warn()); // 1/1 = 100% > 80%
        // Second call blocks
        let r2 = limiter.check_input(&[Message::user("hi")]).await;
        assert!(r2.is_block());
    }
}
