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
