//! OpenAI provider implementation.
//!
//! Supports GPT-4, GPT-4o, GPT-3.5-turbo, and other OpenAI models.
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

/// OpenAI provider configuration.
pub struct OpenAIProvider {
    api_key: String,
    base_url: String,
    client: Client,
    organization: Option<String>,
}

impl OpenAIProvider {
    /// Create a new OpenAI provider with the given API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: "https://api.openai.com/v1".to_string(),
            client: Client::new(),
            organization: None,
        }
    }

    /// Set a custom base URL (for Azure OpenAI, proxies, etc.).
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Set the organization ID.
    pub fn with_organization(mut self, org: impl Into<String>) -> Self {
        self.organization = Some(org.into());
        self
    }

    /// Convert internal messages to OpenAI API format.
    fn convert_messages(messages: &[Message]) -> Vec<Value> {
        messages
            .iter()
            .map(|msg| {
                let role = match msg.role {
                    Role::System => "system",
                    Role::User => "user",
                    Role::Assistant => "assistant",
                    Role::Tool => "tool",
                };

                let mut obj = serde_json::json!({ "role": role });

                // Handle different content types
                let mut text_parts = vec![];
                let mut tool_calls_json = vec![];
                let mut tool_call_id = None;

                for part in &msg.content {
                    match part {
                        ContentPart::Text { text } => {
                            text_parts.push(text.clone());
                        }
                        ContentPart::ToolCall {
                            id,
                            name,
                            arguments,
                        } => {
                            tool_calls_json.push(serde_json::json!({
                                "id": id,
                                "type": "function",
                                "function": {
                                    "name": name,
                                    "arguments": arguments.to_string(),
                                }
                            }));
                        }
                        ContentPart::ToolResult {
                            call_id, content, ..
                        } => {
                            tool_call_id = Some(call_id.clone());
                            text_parts.push(content.to_string());
                        }
                        ContentPart::Image { url, .. } => {
                            // For vision models
                            text_parts.push(format!("[Image: {}]", url));
                        }
                    }
                }

                if !text_parts.is_empty() {
                    obj["content"] = Value::String(text_parts.join("\n"));
                }
                if !tool_calls_json.is_empty() {
                    obj["tool_calls"] = Value::Array(tool_calls_json);
                }
                if let Some(id) = tool_call_id {
                    obj["tool_call_id"] = Value::String(id);
                }

                obj
            })
            .collect()
    }

    /// Convert tool definitions to OpenAI API format.
    fn convert_tools(tools: &[ToolDefinition]) -> Vec<Value> {
        tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    }
                })
            })
            .collect()
    }
}

#[async_trait]
impl Provider for OpenAIProvider {
    fn name(&self) -> &str {
        "openai"
    }

    async fn complete(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse, ProviderError> {
        let url = format!("{}/chat/completions", self.base_url);

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": Self::convert_messages(&request.messages),
            "temperature": request.temperature,
        });

        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = Value::Number(max_tokens.into());
        }

        if !request.tools.is_empty() {
            body["tools"] = Value::Array(Self::convert_tools(&request.tools));
        }

        if !request.stop.is_empty() {
            body["stop"] = Value::Array(
                request
                    .stop
                    .iter()
                    .map(|s| Value::String(s.clone()))
                    .collect(),
            );
        }

        let mut req = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json");

        if let Some(org) = &self.organization {
            req = req.header("OpenAI-Organization", org.as_str());
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

        // Parse the response
        let choice = resp_body
            .get("choices")
            .and_then(|c| c.get(0))
            .ok_or_else(|| ProviderError::InvalidResponse("No choices in response".into()))?;

        let finish_reason = choice
            .get("finish_reason")
            .and_then(|f| f.as_str())
            .map(|f| match f {
                "stop" => FinishReason::Stop,
                "tool_calls" => FinishReason::ToolCalls,
                "length" => FinishReason::Length,
                "content_filter" => FinishReason::ContentFilter,
                _ => FinishReason::Stop,
            });

        let msg = choice
            .get("message")
            .ok_or_else(|| ProviderError::InvalidResponse("No message in choice".into()))?;

        // Build the response message
        let mut content_parts = vec![];

        if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
            content_parts.push(ContentPart::Text {
                text: content.to_string(),
            });
        }

        if let Some(tool_calls) = msg.get("tool_calls").and_then(|tc| tc.as_array()) {
            for tc in tool_calls {
                let id = tc
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let name = tc
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string();
                let arguments_str = tc
                    .get("function")
                    .and_then(|f| f.get("arguments"))
                    .and_then(|a| a.as_str())
                    .unwrap_or("{}");
                let arguments: Value =
                    serde_json::from_str(arguments_str).unwrap_or(Value::Object(Default::default()));

                content_parts.push(ContentPart::ToolCall {
                    id,
                    name,
                    arguments,
                });
            }
        }

        let response_message = Message {
            id: uuid::Uuid::new_v4().to_string(),
            role: Role::Assistant,
            content: content_parts,
            metadata: Default::default(),
            timestamp: chrono::Utc::now(),
        };

        // Parse usage
        let usage = resp_body.get("usage").map(|u| Usage {
            prompt_tokens: u
                .get("prompt_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            completion_tokens: u
                .get("completion_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
            total_tokens: u
                .get("total_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32,
        });

        Ok(CompletionResponse {
            message: response_message,
            usage,
            finish_reason,
            model: request.model,
        })
    }

    fn complete_stream(
        &self,
        request: CompletionRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, ProviderError>> + Send + '_>> {
        let url = format!("{}/chat/completions", self.base_url);
        let api_key = self.api_key.clone();
        let org = self.organization.clone();

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": Self::convert_messages(&request.messages),
            "temperature": request.temperature,
            "stream": true,
        });

        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = Value::Number(max_tokens.into());
        }

        if !request.tools.is_empty() {
            body["tools"] = Value::Array(Self::convert_tools(&request.tools));
        }

        let client = self.client.clone();

        Box::pin(futures::stream::unfold(
            Some((client, url, api_key, org, body)),
            |state| async move {
                let (client, url, api_key, org, body) = state?;

                let mut req = client
                    .post(&url)
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/json");

                if let Some(org) = &org {
                    req = req.header("OpenAI-Organization", org.as_str());
                }

                let resp = match req.json(&body).send().await {
                    Ok(r) => r,
                    Err(e) => {
                        return Some((
                            Err(ProviderError::Network(e.to_string())),
                            None,
                        ));
                    }
                };

                // For SSE streaming, we'd parse the event stream.
                // This is a simplified implementation that returns a Done event.
                let text = match resp.text().await {
                    Ok(t) => t,
                    Err(e) => {
                        return Some((
                            Err(ProviderError::Network(e.to_string())),
                            None,
                        ));
                    }
                };

                // Parse SSE events
                let mut full_content = String::new();
                for line in text.lines() {
                    if let Some(data) = line.strip_prefix("data: ") {
                        if data == "[DONE]" {
                            break;
                        }
                        if let Ok(chunk) = serde_json::from_str::<Value>(data) {
                            if let Some(delta) = chunk
                                .get("choices")
                                .and_then(|c| c.get(0))
                                .and_then(|c| c.get("delta"))
                                .and_then(|d| d.get("content"))
                                .and_then(|c| c.as_str())
                            {
                                full_content.push_str(delta);
                            }
                        }
                    }
                }

                if !full_content.is_empty() {
                    Some((
                        Ok(StreamEvent::ContentDelta {
                            delta: full_content,
                        }),
                        None,
                    ))
                } else {
                    Some((
                        Ok(StreamEvent::Done {
                            usage: None,
                            finish_reason: Some(FinishReason::Stop),
                        }),
                        None,
                    ))
                }
            },
        ))
    }

    fn capabilities(&self) -> ProviderCapabilities {
        ProviderCapabilities {
            streaming: true,
            tool_calling: true,
            vision: true,
            embeddings: true,
            structured_output: true,
        }
    }
}
