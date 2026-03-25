//! Guardrail trait and types — input/output validation and safety mechanisms.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;

use crate::message::Message;

/// Result of a guardrail check.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GuardrailResult {
    /// The check passed.
    Pass,

    /// A warning was generated but execution may continue.
    Warn { message: String },

    /// The check failed and execution should be blocked.
    Block { message: String },
}

impl GuardrailResult {
    pub fn is_pass(&self) -> bool {
        matches!(self, GuardrailResult::Pass)
    }

    pub fn is_block(&self) -> bool {
        matches!(self, GuardrailResult::Block { .. })
    }

    pub fn is_warn(&self) -> bool {
        matches!(self, GuardrailResult::Warn { .. })
    }
}

/// Execution mode for guardrails.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GuardrailMode {
    /// Run guardrails in parallel with the LLM call (best latency).
    #[default]
    Parallel,

    /// Run guardrails before the LLM call (blocks until complete).
    Blocking,
}

/// The core Guardrail trait for input/output validation and safety.
#[async_trait]
pub trait Guardrail: Send + Sync {
    /// Name of this guardrail.
    fn name(&self) -> &str;

    /// Check input messages before they are sent to the LLM.
    async fn check_input(&self, messages: &[Message]) -> GuardrailResult;

    /// Check output message after the LLM generates a response.
    async fn check_output(&self, output: &Message) -> GuardrailResult;

    /// The execution mode for this guardrail.
    fn mode(&self) -> GuardrailMode {
        GuardrailMode::default()
    }
}

impl fmt::Debug for dyn Guardrail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Guardrail")
            .field("name", &self.name())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guardrail_result_pass_serde() {
        let result = GuardrailResult::Pass;
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"type\":\"pass\""));
        let back: GuardrailResult = serde_json::from_str(&json).unwrap();
        assert!(back.is_pass());
        assert!(!back.is_warn());
        assert!(!back.is_block());
    }

    #[test]
    fn guardrail_result_warn_serde() {
        let result = GuardrailResult::Warn {
            message: "caution".into(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"type\":\"warn\""));
        let back: GuardrailResult = serde_json::from_str(&json).unwrap();
        assert!(back.is_warn());
        assert!(!back.is_pass());
        assert!(!back.is_block());
        match back {
            GuardrailResult::Warn { message } => assert_eq!(message, "caution"),
            _ => panic!("expected Warn"),
        }
    }

    #[test]
    fn guardrail_result_block_serde() {
        let result = GuardrailResult::Block {
            message: "denied".into(),
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"type\":\"block\""));
        let back: GuardrailResult = serde_json::from_str(&json).unwrap();
        assert!(back.is_block());
        assert!(!back.is_pass());
        assert!(!back.is_warn());
    }

    #[test]
    fn guardrail_mode_default_is_parallel() {
        let mode = GuardrailMode::default();
        assert_eq!(mode, GuardrailMode::Parallel);
    }

    #[test]
    fn guardrail_mode_equality() {
        assert_eq!(GuardrailMode::Parallel, GuardrailMode::Parallel);
        assert_eq!(GuardrailMode::Blocking, GuardrailMode::Blocking);
        assert_ne!(GuardrailMode::Parallel, GuardrailMode::Blocking);
    }
}
