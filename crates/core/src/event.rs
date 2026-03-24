//! Event system for observability and streaming.
//!
//! RunEvents are emitted during agent execution, enabling real-time streaming,
//! logging, tracing, and debugging.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::provider::Usage;

/// Events emitted during an agent run.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum RunEvent {
    // --- Agent lifecycle events ---
    /// An agent has started processing.
    AgentStart {
        agent: String,
    },

    /// An agent has finished processing.
    AgentEnd {
        agent: String,
    },

    // --- LLM events ---
    /// An LLM call has started.
    LlmStart {
        model: String,
        message_count: usize,
    },

    /// A streaming text chunk from the LLM.
    LlmChunk {
        delta: String,
    },

    /// An LLM call has completed.
    LlmEnd {
        model: String,
        usage: Option<Usage>,
    },

    // --- Tool events ---
    /// A tool call has started.
    ToolStart {
        name: String,
        args: Value,
    },

    /// A tool call has completed.
    ToolEnd {
        name: String,
        output: Value,
        is_error: bool,
    },

    // --- Orchestration events ---
    /// An agent handoff has occurred.
    Handoff {
        from: String,
        to: String,
        reason: String,
    },

    /// A guardrail check was triggered.
    GuardrailTriggered {
        name: String,
        passed: bool,
        message: Option<String>,
    },

    // --- Memory events ---
    /// Messages were stored to memory.
    MemoryStore {
        session_id: String,
        count: usize,
    },

    /// Messages were retrieved from memory.
    MemoryRetrieve {
        session_id: String,
        count: usize,
    },

    // --- Error event ---
    /// An error occurred during the run.
    Error {
        message: String,
    },
}

/// Trait for components that can receive run events.
#[async_trait::async_trait]
pub trait EventHandler: Send + Sync {
    async fn handle(&self, event: &RunEvent);
}

/// A simple event handler that logs events using the `tracing` crate.
pub struct TracingEventHandler;

#[async_trait::async_trait]
impl EventHandler for TracingEventHandler {
    async fn handle(&self, event: &RunEvent) {
        match event {
            RunEvent::AgentStart { agent } => {
                tracing::info!(agent = %agent, "Agent started");
            }
            RunEvent::AgentEnd { agent } => {
                tracing::info!(agent = %agent, "Agent ended");
            }
            RunEvent::LlmStart { model, message_count } => {
                tracing::info!(model = %model, messages = message_count, "LLM call started");
            }
            RunEvent::LlmChunk { delta } => {
                tracing::trace!(delta = %delta, "LLM chunk");
            }
            RunEvent::LlmEnd { model, usage } => {
                tracing::info!(model = %model, ?usage, "LLM call ended");
            }
            RunEvent::ToolStart { name, args } => {
                tracing::info!(tool = %name, %args, "Tool call started");
            }
            RunEvent::ToolEnd { name, is_error, .. } => {
                tracing::info!(tool = %name, is_error = %is_error, "Tool call ended");
            }
            RunEvent::Handoff { from, to, reason } => {
                tracing::info!(from = %from, to = %to, reason = %reason, "Agent handoff");
            }
            RunEvent::GuardrailTriggered { name, passed, message } => {
                tracing::warn!(
                    guardrail = %name,
                    passed = %passed,
                    message = ?message,
                    "Guardrail triggered"
                );
            }
            RunEvent::Error { message } => {
                tracing::error!(error = %message, "Run error");
            }
            _ => {
                tracing::debug!(?event, "Run event");
            }
        }
    }
}
