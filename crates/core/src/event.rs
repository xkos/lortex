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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn agent_start_serde_roundtrip() {
        let event = RunEvent::AgentStart {
            agent: "coder".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"event\":\"agent_start\""));
        let back: RunEvent = serde_json::from_str(&json).unwrap();
        match back {
            RunEvent::AgentStart { agent } => assert_eq!(agent, "coder"),
            _ => panic!("expected AgentStart"),
        }
    }

    #[test]
    fn llm_end_with_usage_serde() {
        let event = RunEvent::LlmEnd {
            model: "gpt-4o".into(),
            usage: Some(Usage {
                prompt_tokens: 100,
                completion_tokens: 50,
                total_tokens: 150,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            }),
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: RunEvent = serde_json::from_str(&json).unwrap();
        match back {
            RunEvent::LlmEnd { model, usage } => {
                assert_eq!(model, "gpt-4o");
                let u = usage.unwrap();
                assert_eq!(u.total_tokens, 150);
            }
            _ => panic!("expected LlmEnd"),
        }
    }

    #[test]
    fn tool_start_serde() {
        let event = RunEvent::ToolStart {
            name: "search".into(),
            args: json!({"query": "rust"}),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"event\":\"tool_start\""));
        let back: RunEvent = serde_json::from_str(&json).unwrap();
        match back {
            RunEvent::ToolStart { name, args } => {
                assert_eq!(name, "search");
                assert_eq!(args, json!({"query": "rust"}));
            }
            _ => panic!("expected ToolStart"),
        }
    }

    #[test]
    fn tool_end_serde() {
        let event = RunEvent::ToolEnd {
            name: "search".into(),
            output: json!("found 3 results"),
            is_error: false,
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: RunEvent = serde_json::from_str(&json).unwrap();
        match back {
            RunEvent::ToolEnd {
                name,
                output,
                is_error,
            } => {
                assert_eq!(name, "search");
                assert_eq!(output, json!("found 3 results"));
                assert!(!is_error);
            }
            _ => panic!("expected ToolEnd"),
        }
    }

    #[test]
    fn handoff_event_serde() {
        let event = RunEvent::Handoff {
            from: "router".into(),
            to: "coder".into(),
            reason: "code task".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: RunEvent = serde_json::from_str(&json).unwrap();
        match back {
            RunEvent::Handoff { from, to, reason } => {
                assert_eq!(from, "router");
                assert_eq!(to, "coder");
                assert_eq!(reason, "code task");
            }
            _ => panic!("expected Handoff"),
        }
    }

    #[test]
    fn guardrail_triggered_serde() {
        let event = RunEvent::GuardrailTriggered {
            name: "toxicity".into(),
            passed: false,
            message: Some("content flagged".into()),
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: RunEvent = serde_json::from_str(&json).unwrap();
        match back {
            RunEvent::GuardrailTriggered {
                name,
                passed,
                message,
            } => {
                assert_eq!(name, "toxicity");
                assert!(!passed);
                assert_eq!(message.as_deref(), Some("content flagged"));
            }
            _ => panic!("expected GuardrailTriggered"),
        }
    }

    #[test]
    fn error_event_serde() {
        let event = RunEvent::Error {
            message: "something failed".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: RunEvent = serde_json::from_str(&json).unwrap();
        match back {
            RunEvent::Error { message } => assert_eq!(message, "something failed"),
            _ => panic!("expected Error"),
        }
    }

    #[test]
    fn memory_events_serde() {
        let store = RunEvent::MemoryStore {
            session_id: "s1".into(),
            count: 5,
        };
        let json = serde_json::to_string(&store).unwrap();
        let back: RunEvent = serde_json::from_str(&json).unwrap();
        match back {
            RunEvent::MemoryStore { session_id, count } => {
                assert_eq!(session_id, "s1");
                assert_eq!(count, 5);
            }
            _ => panic!("expected MemoryStore"),
        }

        let retrieve = RunEvent::MemoryRetrieve {
            session_id: "s1".into(),
            count: 3,
        };
        let json = serde_json::to_string(&retrieve).unwrap();
        let back: RunEvent = serde_json::from_str(&json).unwrap();
        match back {
            RunEvent::MemoryRetrieve { session_id, count } => {
                assert_eq!(session_id, "s1");
                assert_eq!(count, 3);
            }
            _ => panic!("expected MemoryRetrieve"),
        }
    }

    #[test]
    fn llm_chunk_serde() {
        let event = RunEvent::LlmChunk {
            delta: "Hello".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: RunEvent = serde_json::from_str(&json).unwrap();
        match back {
            RunEvent::LlmChunk { delta } => assert_eq!(delta, "Hello"),
            _ => panic!("expected LlmChunk"),
        }
    }
}
