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
    async fn embed(&self, _texts: &[&str]) -> Result<Vec<Vec<f32>>, ProviderError> {
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
