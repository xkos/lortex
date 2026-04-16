//! Provider trait — unified interface for LLM providers.

use async_trait::async_trait;
use futures::Stream;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::pin::Pin;

use crate::error::ProviderError;
use crate::message::Message;

/// A request to an LLM for completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    /// The model identifier (e.g., "gpt-4o", "claude-sonnet-4-20250514").
    pub model: String,

    /// The conversation messages.
    pub messages: Vec<Message>,

    /// Tool definitions available to the model.
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,

    /// Temperature for sampling (0.0 - 2.0).
    #[serde(default = "default_temperature")]
    pub temperature: f32,

    /// Maximum number of tokens to generate.
    pub max_tokens: Option<u32>,

    /// Stop sequences.
    #[serde(default)]
    pub stop: Vec<String>,

    /// Additional provider-specific parameters.
    #[serde(default)]
    pub extra: Value,
}

fn default_temperature() -> f32 {
    0.7
}

/// A tool definition sent to the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value, // JSON Schema
}

/// A response from an LLM completion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    /// The generated message (may contain text and/or tool calls).
    pub message: Message,

    /// Usage statistics.
    pub usage: Option<Usage>,

    /// The finish reason.
    pub finish_reason: Option<FinishReason>,

    /// Model used for this completion.
    pub model: String,
}

/// Token usage statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(default)]
    pub cache_creation_input_tokens: u32,
    #[serde(default)]
    pub cache_read_input_tokens: u32,
}

/// Reason the model stopped generating.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FinishReason {
    Stop,
    ToolCalls,
    Length,
    ContentFilter,
}

/// A streaming event from an LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamEvent {
    /// A chunk of text content.
    ContentDelta { delta: String },

    /// Start of a tool call.
    ToolCallStart {
        index: usize,
        id: String,
        name: String,
    },

    /// A chunk of tool call arguments.
    ToolCallDelta { index: usize, arguments_delta: String },

    /// The stream is complete.
    Done {
        usage: Option<Usage>,
        finish_reason: Option<FinishReason>,
    },
}

// ============================================================================
// Embedding types
// ============================================================================

/// A request to generate embeddings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingRequest {
    /// The model identifier (e.g., "text-embedding-3-small").
    pub model: String,

    /// The input texts to embed.
    pub input: Vec<String>,

    /// Output encoding format: "float" (default) or "base64".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encoding_format: Option<String>,

    /// Output vector dimensions (only supported by some models).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimensions: Option<u32>,
}

/// A response containing embeddings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingResponse {
    /// The embedding vectors.
    pub data: Vec<EmbeddingData>,

    /// The model used.
    pub model: String,

    /// Token usage.
    pub usage: EmbeddingUsage,
}

/// A single embedding vector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingData {
    /// The index of the input text this embedding corresponds to.
    pub index: usize,

    /// The embedding vector (float array) or base64 string.
    pub embedding: Value,
}

/// Token usage for an embedding request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingUsage {
    pub prompt_tokens: u32,
    pub total_tokens: u32,
}

// ============================================================================
// Provider capabilities
// ============================================================================

/// Capabilities that a provider supports.
#[derive(Debug, Clone, Default)]
pub struct ProviderCapabilities {
    pub streaming: bool,
    pub tool_calling: bool,
    pub vision: bool,
    pub embeddings: bool,
    pub structured_output: bool,
}

/// The core Provider trait. Encapsulates interaction with an LLM service.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Provider name (e.g., "openai", "anthropic").
    fn name(&self) -> &str;

    /// Complete a request synchronously.
    async fn complete(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse, ProviderError>;

    /// Complete a request with streaming output.
    fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, ProviderError>> + Send + '_>>;

    /// Generate embeddings for the given texts.
    async fn embed(&self, _request: EmbeddingRequest) -> Result<EmbeddingResponse, ProviderError> {
        Err(ProviderError::ModelNotSupported(
            "Embeddings not supported by this provider".into(),
        ))
    }

    /// Query supported capabilities.
    fn capabilities(&self) -> ProviderCapabilities;
}

impl fmt::Debug for dyn Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Provider")
            .field("name", &self.name())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn completion_request_serde_roundtrip() {
        let req = CompletionRequest {
            model: "gpt-4o".into(),
            messages: vec![Message::user("hello")],
            tools: vec![ToolDefinition {
                name: "search".into(),
                description: "Search the web".into(),
                parameters: json!({"type":"object"}),
            }],
            temperature: 0.5,
            max_tokens: Some(1024),
            stop: vec!["END".into()],
            extra: json!({}),
        };
        let json_str = serde_json::to_string(&req).unwrap();
        let back: CompletionRequest = serde_json::from_str(&json_str).unwrap();
        assert_eq!(back.model, "gpt-4o");
        assert_eq!(back.messages.len(), 1);
        assert_eq!(back.tools.len(), 1);
        assert_eq!(back.tools[0].name, "search");
        assert_eq!(back.temperature, 0.5);
        assert_eq!(back.max_tokens, Some(1024));
        assert_eq!(back.stop, vec!["END"]);
    }

    #[test]
    fn completion_request_default_temperature() {
        let json_str = r#"{"model":"gpt-4o","messages":[]}"#;
        let req: CompletionRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(req.temperature, 0.7);
        assert!(req.tools.is_empty());
        assert!(req.stop.is_empty());
    }

    #[test]
    fn usage_serde_roundtrip() {
        let usage = Usage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: 0,
        };
        let json_str = serde_json::to_string(&usage).unwrap();
        let back: Usage = serde_json::from_str(&json_str).unwrap();
        assert_eq!(back.prompt_tokens, 100);
        assert_eq!(back.completion_tokens, 50);
        assert_eq!(back.total_tokens, 150);
    }

    #[test]
    fn finish_reason_serde() {
        for (reason, expected) in [
            (FinishReason::Stop, "\"stop\""),
            (FinishReason::ToolCalls, "\"tool_calls\""),
            (FinishReason::Length, "\"length\""),
            (FinishReason::ContentFilter, "\"content_filter\""),
        ] {
            let json = serde_json::to_string(&reason).unwrap();
            assert_eq!(json, expected);
            let back: FinishReason = serde_json::from_str(&json).unwrap();
            assert_eq!(back, reason);
        }
    }

    #[test]
    fn stream_event_content_delta_serde() {
        let event = StreamEvent::ContentDelta {
            delta: "Hello".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"type\":\"content_delta\""));
        let back: StreamEvent = serde_json::from_str(&json).unwrap();
        match back {
            StreamEvent::ContentDelta { delta } => assert_eq!(delta, "Hello"),
            _ => panic!("expected ContentDelta"),
        }
    }

    #[test]
    fn stream_event_tool_call_start_serde() {
        let event = StreamEvent::ToolCallStart {
            index: 0,
            id: "call_1".into(),
            name: "search".into(),
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: StreamEvent = serde_json::from_str(&json).unwrap();
        match back {
            StreamEvent::ToolCallStart { index, id, name } => {
                assert_eq!(index, 0);
                assert_eq!(id, "call_1");
                assert_eq!(name, "search");
            }
            _ => panic!("expected ToolCallStart"),
        }
    }

    #[test]
    fn stream_event_done_with_usage() {
        let event = StreamEvent::Done {
            usage: Some(Usage {
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            }),
            finish_reason: Some(FinishReason::Stop),
        };
        let json = serde_json::to_string(&event).unwrap();
        let back: StreamEvent = serde_json::from_str(&json).unwrap();
        match back {
            StreamEvent::Done {
                usage,
                finish_reason,
            } => {
                assert_eq!(usage.unwrap().total_tokens, 30);
                assert_eq!(finish_reason, Some(FinishReason::Stop));
            }
            _ => panic!("expected Done"),
        }
    }

    #[test]
    fn provider_capabilities_default() {
        let caps = ProviderCapabilities::default();
        assert!(!caps.streaming);
        assert!(!caps.tool_calling);
        assert!(!caps.vision);
        assert!(!caps.embeddings);
        assert!(!caps.structured_output);
    }

    #[test]
    fn embedding_request_serde_roundtrip() {
        let req = EmbeddingRequest {
            model: "text-embedding-3-small".into(),
            input: vec!["hello".into(), "world".into()],
            encoding_format: Some("float".into()),
            dimensions: Some(256),
        };
        let json_str = serde_json::to_string(&req).unwrap();
        let back: EmbeddingRequest = serde_json::from_str(&json_str).unwrap();
        assert_eq!(back.model, "text-embedding-3-small");
        assert_eq!(back.input.len(), 2);
        assert_eq!(back.encoding_format.as_deref(), Some("float"));
        assert_eq!(back.dimensions, Some(256));
    }

    #[test]
    fn embedding_request_minimal() {
        let json_str = r#"{"model":"text-embedding-3-small","input":["test"]}"#;
        let req: EmbeddingRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(req.model, "text-embedding-3-small");
        assert!(req.encoding_format.is_none());
        assert!(req.dimensions.is_none());
    }

    #[test]
    fn embedding_response_serde_roundtrip() {
        let resp = EmbeddingResponse {
            data: vec![EmbeddingData {
                index: 0,
                embedding: json!([0.1, 0.2, 0.3]),
            }],
            model: "text-embedding-3-small".into(),
            usage: EmbeddingUsage {
                prompt_tokens: 5,
                total_tokens: 5,
            },
        };
        let json_str = serde_json::to_string(&resp).unwrap();
        let back: EmbeddingResponse = serde_json::from_str(&json_str).unwrap();
        assert_eq!(back.data.len(), 1);
        assert_eq!(back.data[0].index, 0);
        assert_eq!(back.model, "text-embedding-3-small");
        assert_eq!(back.usage.prompt_tokens, 5);
        assert_eq!(back.usage.total_tokens, 5);
    }

    #[test]
    fn embedding_data_base64_format() {
        let data = EmbeddingData {
            index: 0,
            embedding: json!("AAAAAAAAAIA/AAAAQAAAQEA="),
        };
        let json_str = serde_json::to_string(&data).unwrap();
        let back: EmbeddingData = serde_json::from_str(&json_str).unwrap();
        assert!(back.embedding.is_string());
    }

    #[test]
    fn tool_definition_serde_roundtrip() {
        let def = ToolDefinition {
            name: "calc".into(),
            description: "Calculator".into(),
            parameters: json!({"type":"object","properties":{"expr":{"type":"string"}}}),
        };
        let json_str = serde_json::to_string(&def).unwrap();
        let back: ToolDefinition = serde_json::from_str(&json_str).unwrap();
        assert_eq!(back.name, "calc");
        assert_eq!(back.description, "Calculator");
        assert_eq!(back.parameters["type"], "object");
    }
}
