//! Anthropic provider implementation.
//!
//! Supports Claude 4 Opus, Claude 4 Sonnet, Claude Haiku, and other Anthropic models.
//! Implements both synchronous and streaming completion.

use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use reqwest::Client;
use serde_json::Value;

use lortex_core::error::ProviderError;
use lortex_core::message::{ContentPart, Message, Role};
use lortex_core::provider::{
    CompletionRequest, CompletionResponse, FinishReason, Provider, ProviderCapabilities,
    StreamEvent, ToolDefinition, Usage,
};

const ANTHROPIC_API_VERSION: &str = "2023-06-01";

/// Anthropic provider configuration.
pub struct AnthropicProvider {
    api_key: String,
    base_url: String,
    client: Client,
    extra_headers: std::collections::HashMap<String, String>,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider with the given API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: "https://api.anthropic.com/v1".to_string(),
            client: Client::new(),
            extra_headers: std::collections::HashMap::new(),
        }
    }

    /// Set a custom base URL.
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Add extra headers to all requests.
    pub fn with_extra_headers(mut self, headers: std::collections::HashMap<String, String>) -> Self {
        self.extra_headers = headers;
        self
    }

    /// Convert internal messages to Anthropic API format.
    /// Anthropic uses a different format: system is a top-level parameter, not a message.
    fn convert_messages(messages: &[Message]) -> (Option<String>, Vec<Value>) {
        let mut system_prompt = None;
        let mut api_messages = vec![];

        for msg in messages {
            match msg.role {
                Role::System => {
                    if let Some(text) = msg.text() {
                        system_prompt = Some(text.to_string());
                    }
                }
                Role::User => {
                    let mut content_blocks = vec![];
                    for part in &msg.content {
                        match part {
                            ContentPart::Text { text } => {
                                content_blocks.push(serde_json::json!({
                                    "type": "text",
                                    "text": text,
                                }));
                            }
                            ContentPart::Image { url, media_type: _ } => {
                                content_blocks.push(serde_json::json!({
                                    "type": "image",
                                    "source": {
                                        "type": "url",
                                        "url": url,
                                    }
                                }));
                            }
                            _ => {}
                        }
                    }
                    api_messages.push(serde_json::json!({
                        "role": "user",
                        "content": content_blocks,
                    }));
                }
                Role::Assistant => {
                    let mut content_blocks = vec![];
                    for part in &msg.content {
                        match part {
                            ContentPart::Text { text } => {
                                content_blocks.push(serde_json::json!({
                                    "type": "text",
                                    "text": text,
                                }));
                            }
                            ContentPart::ToolCall {
                                id,
                                name,
                                arguments,
                            } => {
                                content_blocks.push(serde_json::json!({
                                    "type": "tool_use",
                                    "id": id,
                                    "name": name,
                                    "input": arguments,
                                }));
                            }
                            _ => {}
                        }
                    }
                    api_messages.push(serde_json::json!({
                        "role": "assistant",
                        "content": content_blocks,
                    }));
                }
                Role::Tool => {
                    let mut content_blocks = vec![];
                    for part in &msg.content {
                        if let ContentPart::ToolResult {
                            call_id,
                            content,
                            is_error,
                        } = part
                        {
                            content_blocks.push(serde_json::json!({
                                "type": "tool_result",
                                "tool_use_id": call_id,
                                "content": content.to_string(),
                                "is_error": is_error,
                            }));
                        }
                    }
                    api_messages.push(serde_json::json!({
                        "role": "user",
                        "content": content_blocks,
                    }));
                }
            }
        }

        (system_prompt, api_messages)
    }

    /// Convert tool definitions to Anthropic API format.
    fn convert_tools(tools: &[ToolDefinition]) -> Vec<Value> {
        tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.parameters,
                })
            })
            .collect()
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn complete(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse, ProviderError> {
        let url = format!("{}/messages", self.base_url);

        let (system_prompt, messages) = Self::convert_messages(&request.messages);

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": messages,
            "max_tokens": request.max_tokens.unwrap_or(4096),
            "temperature": request.temperature,
        });

        if let Some(system) = &system_prompt {
            body["system"] = Value::String(system.clone());
        }

        if !request.tools.is_empty() {
            body["tools"] = Value::Array(Self::convert_tools(&request.tools));
        }

        let mut req = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_API_VERSION)
            .header("Content-Type", "application/json");
        for (k, v) in &self.extra_headers {
            req = req.header(k.as_str(), v.as_str());
        }
        let resp = req
            .json(&body)
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let status = resp.status().as_u16();
        if status == 429 {
            return Err(ProviderError::RateLimited {
                retry_after_ms: 1000,
            });
        }
        if status == 401 {
            return Err(ProviderError::AuthenticationFailed(
                "Invalid API key".into(),
            ));
        }

        let resp_body: Value = resp
            .json()
            .await
            .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

        if status >= 400 {
            let message = resp_body
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error")
                .to_string();
            return Err(ProviderError::Api { status, message });
        }

        // Parse content blocks
        let mut content_parts = vec![];
        if let Some(content_blocks) = resp_body.get("content").and_then(|c| c.as_array()) {
            for block in content_blocks {
                let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                match block_type {
                    "text" => {
                        if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                            content_parts.push(ContentPart::Text {
                                text: text.to_string(),
                            });
                        }
                    }
                    "tool_use" => {
                        let id = block
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let name = block
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let input = block.get("input").cloned().unwrap_or(Value::Object(Default::default()));
                        content_parts.push(ContentPart::ToolCall {
                            id,
                            name,
                            arguments: input,
                        });
                    }
                    _ => {}
                }
            }
        }

        let response_message = Message {
            id: resp_body
                .get("id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            role: Role::Assistant,
            content: content_parts,
            metadata: Default::default(),
            timestamp: chrono::Utc::now(),
        };

        let stop_reason = resp_body
            .get("stop_reason")
            .and_then(|s| s.as_str())
            .map(|s| match s {
                "end_turn" => FinishReason::Stop,
                "tool_use" => FinishReason::ToolCalls,
                "max_tokens" => FinishReason::Length,
                _ => FinishReason::Stop,
            });

        let usage = resp_body.get("usage").map(|u| Usage {
            prompt_tokens: u
                .get("input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            completion_tokens: u
                .get("output_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            total_tokens: (u
                .get("input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                + u.get("output_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)) as u32,
        });

        Ok(CompletionResponse {
            message: response_message,
            usage,
            finish_reason: stop_reason,
            model: request.model,
        })
    }

    fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, ProviderError>> + Send + '_>> {
        let url = format!("{}/messages", self.base_url);
        let api_key = self.api_key.clone();
        let (system_prompt, messages) = Self::convert_messages(&request.messages);

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": messages,
            "max_tokens": request.max_tokens.unwrap_or(4096),
            "temperature": request.temperature,
            "stream": true,
        });

        if let Some(system) = &system_prompt {
            body["system"] = Value::String(system.clone());
        }

        if !request.tools.is_empty() {
            body["tools"] = Value::Array(Self::convert_tools(&request.tools));
        }

        let client = self.client.clone();
        let extra_headers = self.extra_headers.clone();

        Box::pin(async_stream::try_stream! {
            let mut req = client
                .post(&url)
                .header("x-api-key", &api_key)
                .header("anthropic-version", ANTHROPIC_API_VERSION)
                .header("Content-Type", "application/json");
            for (k, v) in &extra_headers {
                req = req.header(k.as_str(), v.as_str());
            }
            let resp = req
                .json(&body)
                .send()
                .await
                .map_err(|e| ProviderError::Network(e.to_string()))?;

            let status = resp.status().as_u16();
            if status >= 400 {
                let text = resp.text().await.unwrap_or_default();
                return Err(ProviderError::Api { status, message: text })?;
            }

            let mut byte_stream = resp.bytes_stream();
            let mut buffer = String::new();
            let mut current_tool_index: usize = 0;
            let mut usage_data: Option<Usage> = None;

            use futures::StreamExt;
            while let Some(chunk) = byte_stream.next().await {
                let chunk = chunk.map_err(|e| ProviderError::Network(e.to_string()))?;
                buffer.push_str(&String::from_utf8_lossy(&chunk));

                while let Some(newline_pos) = buffer.find('\n') {
                    let line = buffer[..newline_pos].trim_end_matches('\r').to_string();
                    buffer = buffer[newline_pos + 1..].to_string();

                    if line.is_empty() {
                        continue;
                    }

                    let data = match line.strip_prefix("data: ") {
                        Some(d) => d,
                        None => continue,
                    };

                    let event: Value = match serde_json::from_str(data) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");

                    match event_type {
                        "content_block_start" => {
                            let index = event.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                            if let Some(block) = event.get("content_block") {
                                let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                                if block_type == "tool_use" {
                                    current_tool_index = index;
                                    let id = block.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    let name = block.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                                    yield StreamEvent::ToolCallStart { index, id, name };
                                }
                            }
                        }
                        "content_block_delta" => {
                            if let Some(delta) = event.get("delta") {
                                let delta_type = delta.get("type").and_then(|t| t.as_str()).unwrap_or("");
                                match delta_type {
                                    "text_delta" => {
                                        if let Some(text) = delta.get("text").and_then(|t| t.as_str()) {
                                            if !text.is_empty() {
                                                yield StreamEvent::ContentDelta { delta: text.to_string() };
                                            }
                                        }
                                    }
                                    "input_json_delta" => {
                                        if let Some(partial) = delta.get("partial_json").and_then(|p| p.as_str()) {
                                            if !partial.is_empty() {
                                                yield StreamEvent::ToolCallDelta {
                                                    index: current_tool_index,
                                                    arguments_delta: partial.to_string(),
                                                };
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        "message_delta" => {
                            let stop_reason = event
                                .get("delta")
                                .and_then(|d| d.get("stop_reason"))
                                .and_then(|s| s.as_str());

                            // Capture usage from message_delta
                            if let Some(u) = event.get("usage") {
                                let output_tokens = u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                                if let Some(ref mut existing) = usage_data {
                                    existing.completion_tokens = output_tokens;
                                    existing.total_tokens = existing.prompt_tokens + output_tokens;
                                }
                            }

                            if let Some(reason) = stop_reason {
                                let finish_reason = match reason {
                                    "end_turn" => FinishReason::Stop,
                                    "tool_use" => FinishReason::ToolCalls,
                                    "max_tokens" => FinishReason::Length,
                                    _ => FinishReason::Stop,
                                };
                                yield StreamEvent::Done {
                                    usage: usage_data.clone(),
                                    finish_reason: Some(finish_reason),
                                };
                            }
                        }
                        "message_start" => {
                            // Capture initial usage (input tokens)
                            if let Some(msg) = event.get("message") {
                                if let Some(u) = msg.get("usage") {
                                    let input_tokens = u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                                    usage_data = Some(Usage {
                                        prompt_tokens: input_tokens,
                                        completion_tokens: 0,
                                        total_tokens: input_tokens,
                                    });
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        })
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            tool_calling: true,
            vision: true,
            embeddings: false, // Anthropic doesn't offer embeddings
            structured_output: true,
        }
    }
}
