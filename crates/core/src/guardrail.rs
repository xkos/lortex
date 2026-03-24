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
