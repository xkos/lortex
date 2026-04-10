//! Token budget guardrail — limits token consumption.

use std::sync::atomic::{AtomicU32, Ordering};

use async_trait::async_trait;

use lortex_core::guardrail::{Guardrail, GuardrailResult};
use lortex_core::message::Message;

/// A guardrail that tracks and limits token consumption.
///
/// Uses a simple heuristic: ~4 characters per token (English text).
pub struct TokenBudget {
    /// Maximum allowed tokens.
    max_tokens: u32,

    /// Tokens consumed so far.
    consumed: AtomicU32,
}

impl TokenBudget {
    /// Create a new token budget guardrail.
    pub fn new(max_tokens: u32) -> Self {
        Self {
            max_tokens,
            consumed: AtomicU32::new(0),
        }
    }

    /// Estimate tokens in a string (~4 chars per token).
    fn estimate_tokens(text: &str) -> u32 {
        (text.len() as f64 / 4.0).ceil() as u32
    }

    /// Get the remaining token budget.
    pub fn remaining(&self) -> u32 {
        let consumed = self.consumed.load(Ordering::Relaxed);
        self.max_tokens.saturating_sub(consumed)
    }

    /// Reset the consumed counter.
    pub fn reset(&self) {
        self.consumed.store(0, Ordering::Relaxed);
    }
}

#[async_trait]
impl Guardrail for TokenBudget {
    fn name(&self) -> &str {
        "token_budget"
    }

    async fn check_input(&self, messages: &[Message]) -> GuardrailResult {
        let mut total_tokens = 0u32;
        for msg in messages {
            if let Some(text) = msg.text() {
                total_tokens += Self::estimate_tokens(text);
            }
        }

        let consumed = self.consumed.fetch_add(total_tokens, Ordering::Relaxed);
        let new_total = consumed + total_tokens;

        if new_total > self.max_tokens {
            GuardrailResult::Block {
                message: format!(
                    "Token budget exceeded: {} / {} tokens used",
                    new_total, self.max_tokens
                ),
            }
        } else if new_total as f64 > self.max_tokens as f64 * 0.8 {
            GuardrailResult::Warn {
                message: format!(
                    "Token budget warning: {} / {} tokens used (80%+)",
                    new_total, self.max_tokens
                ),
            }
        } else {
            GuardrailResult::Pass
        }
    }

    async fn check_output(&self, output: &Message) -> GuardrailResult {
        if let Some(text) = output.text() {
            let tokens = Self::estimate_tokens(text);
            self.consumed.fetch_add(tokens, Ordering::Relaxed);
        }
        GuardrailResult::Pass
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lortex_core::guardrail::Guardrail;
    use lortex_core::message::Message;

    #[test]
    fn name_returns_token_budget() {
        let budget = TokenBudget::new(1000);
        assert_eq!(budget.name(), "token_budget");
    }

    #[test]
    fn remaining_starts_at_max() {
        let budget = TokenBudget::new(1000);
        assert_eq!(budget.remaining(), 1000);
    }

    #[tokio::test]
    async fn passes_under_budget() {
        let budget = TokenBudget::new(1000);
        let result = budget.check_input(&[Message::user("short")]).await;
        assert!(result.is_pass());
        assert!(budget.remaining() < 1000);
    }

    #[tokio::test]
    async fn warns_at_80_percent() {
        // 100 token budget, ~4 chars per token
        // 324 chars = ceil(324/4) = 81 tokens = 81% > 80%
        let budget = TokenBudget::new(100);
        let long_text = "a".repeat(324);
        let result = budget.check_input(&[Message::user(&long_text)]).await;
        assert!(result.is_warn());
    }

    #[tokio::test]
    async fn blocks_over_budget() {
        let budget = TokenBudget::new(10);
        // 100 chars = ~25 tokens, well over 10
        let long_text = "a".repeat(100);
        let result = budget.check_input(&[Message::user(&long_text)]).await;
        assert!(result.is_block());
    }

    #[tokio::test]
    async fn accumulates_across_calls() {
        let budget = TokenBudget::new(100);
        // Each call adds some tokens
        budget
            .check_input(&[Message::user(&"a".repeat(200))])
            .await; // ~50 tokens
        budget
            .check_input(&[Message::user(&"b".repeat(200))])
            .await; // ~50 more
        // Now at ~100 tokens, next should block
        let result = budget
            .check_input(&[Message::user(&"c".repeat(100))])
            .await;
        assert!(result.is_block());
    }

    #[tokio::test]
    async fn check_output_tracks_tokens() {
        let budget = TokenBudget::new(1000);
        let initial = budget.remaining();
        budget
            .check_output(&Message::assistant("some output text"))
            .await;
        assert!(budget.remaining() < initial);
    }

    #[tokio::test]
    async fn check_output_always_passes() {
        let budget = TokenBudget::new(1);
        // Even when over budget, check_output returns Pass
        budget
            .check_input(&[Message::user(&"a".repeat(100))])
            .await;
        let result = budget
            .check_output(&Message::assistant("more text"))
            .await;
        assert!(result.is_pass());
    }

    #[test]
    fn reset_clears_consumed() {
        let budget = TokenBudget::new(1000);
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(budget.check_input(&[Message::user(&"a".repeat(400))]));
        assert!(budget.remaining() < 1000);
        budget.reset();
        assert_eq!(budget.remaining(), 1000);
    }

    #[tokio::test]
    async fn multiple_messages_in_single_check() {
        let budget = TokenBudget::new(1000);
        let msgs = vec![
            Message::user(&"a".repeat(100)),
            Message::user(&"b".repeat(100)),
        ];
        budget.check_input(&msgs).await;
        // 100 chars = ceil(100/4) = 25 tokens each, 50 total
        // remaining = 1000 - 50 = 950
        assert!(budget.remaining() <= 950);
    }
}
