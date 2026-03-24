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
