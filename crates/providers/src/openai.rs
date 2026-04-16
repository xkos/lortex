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

use crate::CacheStrategy;

/// OpenAI provider configuration.
pub struct OpenAIProvider {
    api_key: String,
    base_url: String,
    client: Client,
    organization: Option<String>,
    extra_headers: std::collections::HashMap<String, String>,
    cache_strategy: CacheStrategy,
}

impl OpenAIProvider {
    /// Create a new OpenAI provider with the given API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: "https://api.openai.com/v1".to_string(),
            client: Client::new(),
            organization: None,
            extra_headers: std::collections::HashMap::new(),
            cache_strategy: CacheStrategy::None,
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

    /// Add extra headers to all requests.
    pub fn with_extra_headers(mut self, headers: std::collections::HashMap<String, String>) -> Self {
        self.extra_headers = headers;
        self
    }

    /// Set cache strategy for automatic cache_control injection.
    pub fn with_cache_strategy(mut self, strategy: CacheStrategy) -> Self {
        self.cache_strategy = strategy;
        self
    }

    /// 在已构建的请求 body JSON 上注入 cache_control breakpoint。
    fn inject_cache_breakpoints(body: &mut Value, strategy: CacheStrategy) {
        if strategy == CacheStrategy::None {
            return;
        }
        tracing::debug!(strategy = strategy.as_str(), "Injecting cache breakpoints (OpenAI)");
        let ephemeral = serde_json::json!({"type": "ephemeral"});

        // 1. System message（所有非 None 策略）：找最后一条 system role，content 从 string 转 blocks
        if let Some(Value::Array(messages)) = body.get_mut("messages") {
            if let Some(sys_idx) = messages
                .iter()
                .rposition(|m| m.get("role").and_then(|r| r.as_str()) == Some("system"))
            {
                let sys_msg = &mut messages[sys_idx];
                if let Some(Value::String(text)) = sys_msg.get("content").cloned() {
                    sys_msg["content"] = serde_json::json!([{
                        "type": "text",
                        "text": text,
                        "cache_control": ephemeral,
                    }]);
                } else if let Some(Value::Array(blocks)) = sys_msg.get_mut("content") {
                    if let Some(last) = blocks.last_mut() {
                        if last.get("cache_control").is_none() {
                            last["cache_control"] = ephemeral.clone();
                        }
                    }
                }
            }
        }

        if strategy == CacheStrategy::SystemOnly {
            return;
        }

        // 2. Tools 最后一个（Standard + Full）
        if let Some(Value::Array(tools)) = body.get_mut("tools") {
            if let Some(last_tool) = tools.last_mut() {
                if last_tool.get("cache_control").is_none() {
                    last_tool["cache_control"] = ephemeral.clone();
                }
            }
        }

        if strategy == CacheStrategy::Standard {
            return;
        }

        // 3. Messages 倒数第二条 user 消息（Full）
        // 先检查 system/tools 存在性
        let has_system = body
            .get("messages")
            .and_then(|m| m.as_array())
            .map(|msgs| msgs.iter().any(|m| m.get("role").and_then(|r| r.as_str()) == Some("system")))
            .unwrap_or(false);
        let has_tools = body.get("tools").is_some();

        if let Some(Value::Array(messages)) = body.get_mut("messages") {
            let user_indices: Vec<usize> = messages
                .iter()
                .enumerate()
                .filter(|(_, m)| m.get("role").and_then(|r| r.as_str()) == Some("user"))
                .map(|(i, _)| i)
                .collect();

            let target_idx = if user_indices.len() >= 2 {
                Some(user_indices[user_indices.len() - 2])
            } else if user_indices.len() == 1 && (has_system || has_tools) {
                Some(user_indices[0])
            } else {
                Option::None
            };

            if let Some(idx) = target_idx {
                let msg = &mut messages[idx];
                match msg.get("content").cloned() {
                    Some(Value::String(text)) => {
                        msg["content"] = serde_json::json!([{
                            "type": "text",
                            "text": text,
                            "cache_control": ephemeral,
                        }]);
                    }
                    Some(Value::Array(_)) => {
                        if let Some(Value::Array(blocks)) = msg.get_mut("content") {
                            if let Some(last) = blocks.last_mut() {
                                if last.get("cache_control").is_none() {
                                    last["cache_control"] = ephemeral;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
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

    /// Reassemble a complete response from SSE chunks.
    /// Some providers return SSE format even for non-streaming requests.
    fn reassemble_sse_response(sse_text: &str) -> Result<Value, ProviderError> {
        let mut content = String::new();
        let mut model = String::new();
        let mut finish_reason = None;
        let mut usage = None;
        let mut id = String::new();
        let mut tool_args: std::collections::HashMap<usize, (String, String, String)> = std::collections::HashMap::new();

        for line in sse_text.lines() {
            let data = match line.strip_prefix("data: ") {
                Some(d) if d != "[DONE]" => d,
                _ => continue,
            };
            let chunk: Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => continue,
            };

            if id.is_empty() {
                if let Some(cid) = chunk.get("id").and_then(|v| v.as_str()) {
                    id = cid.to_string();
                }
            }
            if model.is_empty() {
                if let Some(m) = chunk.get("model").and_then(|v| v.as_str()) {
                    model = m.to_string();
                }
            }

            if let Some(choice) = chunk.get("choices").and_then(|c| c.get(0)) {
                if let Some(delta) = choice.get("delta").and_then(|d| d.get("content")).and_then(|c| c.as_str()) {
                    content.push_str(delta);
                }
                if let Some(tcs) = choice.get("delta").and_then(|d| d.get("tool_calls")).and_then(|t| t.as_array()) {
                    for tc in tcs {
                        let idx = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                        if let Some(tc_id) = tc.get("id").and_then(|v| v.as_str()) {
                            let name = tc.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()).unwrap_or("").to_string();
                            tool_args.insert(idx, (tc_id.to_string(), name, String::new()));
                        }
                        if let Some(args) = tc.get("function").and_then(|f| f.get("arguments")).and_then(|a| a.as_str()) {
                            if let Some(entry) = tool_args.get_mut(&idx) {
                                entry.2.push_str(args);
                            }
                        }
                    }
                }
                if let Some(fr) = choice.get("finish_reason").and_then(|f| f.as_str()) {
                    finish_reason = Some(fr.to_string());
                }
            }

            if let Some(u) = chunk.get("usage") {
                if u.get("total_tokens").and_then(|t| t.as_u64()).unwrap_or(0) > 0 {
                    usage = Some(u.clone());
                }
            }
        }

        let mut tool_calls = Vec::new();
        let mut sorted_indices: Vec<usize> = tool_args.keys().cloned().collect();
        sorted_indices.sort();
        for idx in sorted_indices {
            let (tc_id, name, args) = &tool_args[&idx];
            tool_calls.push(serde_json::json!({
                "id": tc_id,
                "type": "function",
                "function": { "name": name, "arguments": args }
            }));
        }

        let mut message = serde_json::json!({"role": "assistant"});
        if !content.is_empty() {
            message["content"] = Value::String(content);
        }
        if !tool_calls.is_empty() {
            message["tool_calls"] = Value::Array(tool_calls);
        }

        let mut resp = serde_json::json!({
            "id": id,
            "object": "chat.completion",
            "model": model,
            "choices": [{"index": 0, "message": message, "finish_reason": finish_reason}],
        });
        if let Some(u) = usage {
            resp["usage"] = u;
        }

        Ok(resp)
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

        Self::inject_cache_breakpoints(&mut body, self.cache_strategy);

        let mut req = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json");

        if let Some(org) = &self.organization {
            req = req.header("OpenAI-Organization", org.as_str());
        }
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

        let resp_text = resp
            .text()
            .await
            .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

        tracing::debug!(
            response_body = %resp_text.chars().take(500).collect::<String>(),
            "OpenAI provider raw response"
        );

        // Try direct JSON parse first; if it fails and the response looks like SSE,
        // reassemble from SSE chunks (some providers return SSE even for non-streaming requests)
        let resp_body: Value = match serde_json::from_str(&resp_text) {
            Ok(v) => v,
            Err(_) if resp_text.starts_with("data: ") => {
                Self::reassemble_sse_response(&resp_text)?
            }
            Err(e) => {
                return Err(ProviderError::InvalidResponse(format!(
                    "Failed to parse JSON: {}. Body starts with: {}",
                    e,
                    resp_text.chars().take(200).collect::<String>()
                )));
            }
        };

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
        let usage = resp_body.get("usage").map(|u| {
            let cached = u.get("prompt_tokens_details")
                .and_then(|d| d.get("cached_tokens"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            Usage {
                prompt_tokens: u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                completion_tokens: u.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                total_tokens: u.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: cached,
            }
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
        let extra_headers = self.extra_headers.clone();

        let mut body = serde_json::json!({
            "model": request.model,
            "messages": Self::convert_messages(&request.messages),
            "temperature": request.temperature,
            "stream": true,
            "stream_options": {"include_usage": true},
        });

        if let Some(max_tokens) = request.max_tokens {
            body["max_tokens"] = Value::Number(max_tokens.into());
        }

        if !request.tools.is_empty() {
            body["tools"] = Value::Array(Self::convert_tools(&request.tools));
        }

        Self::inject_cache_breakpoints(&mut body, self.cache_strategy);

        let client = self.client.clone();

        Box::pin(async_stream::try_stream! {
            let mut req = client
                .post(&url)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json");

            if let Some(org) = &org {
                req = req.header("OpenAI-Organization", org.as_str());
            }
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
            // OpenAI sends finish_reason and usage in separate chunks:
            //   chunk N:   finish_reason: "stop"
            //   chunk N+1: usage: { prompt_tokens, completion_tokens, ... }
            //   [DONE]
            // We defer emitting Done until we have both (or hit [DONE]).
            let mut pending_finish: Option<FinishReason> = None;
            let mut usage_data: Option<Usage> = None;

            use futures::StreamExt;
            while let Some(chunk) = byte_stream.next().await {
                let chunk = chunk.map_err(|e| ProviderError::Network(e.to_string()))?;
                buffer.push_str(&String::from_utf8_lossy(&chunk));

                // Process complete SSE lines from buffer
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

                    if data == "[DONE]" {
                        // Flush pending Done if finish_reason was seen
                        if let Some(fr) = pending_finish.take() {
                            yield StreamEvent::Done {
                                usage: usage_data.take(),
                                finish_reason: Some(fr),
                            };
                        }
                        return;
                    }

                    let chunk_json: Value = match serde_json::from_str(data) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    tracing::trace!(
                        chunk = %serde_json::to_string(&chunk_json).unwrap_or_default(),
                        "OpenAI SSE chunk"
                    );

                    // Parse content delta
                    if let Some(delta) = chunk_json
                        .get("choices")
                        .and_then(|c| c.get(0))
                        .and_then(|c| c.get("delta"))
                        .and_then(|d| d.get("content"))
                        .and_then(|c| c.as_str())
                    {
                        if !delta.is_empty() {
                            yield StreamEvent::ContentDelta { delta: delta.to_string() };
                        }
                    }

                    // Parse tool call deltas
                    if let Some(tool_calls) = chunk_json
                        .get("choices")
                        .and_then(|c| c.get(0))
                        .and_then(|c| c.get("delta"))
                        .and_then(|d| d.get("tool_calls"))
                        .and_then(|tc| tc.as_array())
                    {
                        for tc in tool_calls {
                            let index = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                            if let Some(id) = tc.get("id").and_then(|v| v.as_str()) {
                                let name = tc.get("function")
                                    .and_then(|f| f.get("name"))
                                    .and_then(|n| n.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                tracing::debug!(
                                    tool_index = index,
                                    tool_id = %id,
                                    tool_name = %name,
                                    "OpenAI SSE: ToolCallStart"
                                );
                                yield StreamEvent::ToolCallStart {
                                    index,
                                    id: id.to_string(),
                                    name,
                                };
                            }
                            if let Some(args) = tc.get("function")
                                .and_then(|f| f.get("arguments"))
                                .and_then(|a| a.as_str())
                            {
                                if !args.is_empty() {
                                    yield StreamEvent::ToolCallDelta {
                                        index,
                                        arguments_delta: args.to_string(),
                                    };
                                }
                            }
                        }
                    }

                    // Parse usage (may arrive in finish chunk or a separate chunk)
                    if let Some(u) = chunk_json.get("usage") {
                        let cached = u.get("prompt_tokens_details")
                            .and_then(|d| d.get("cached_tokens"))
                            .and_then(|v| v.as_u64())
                            .unwrap_or(0) as u32;
                        usage_data = Some(Usage {
                            prompt_tokens: u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                            completion_tokens: u.get("completion_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                            total_tokens: u.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32,
                            cache_creation_input_tokens: 0,
                            cache_read_input_tokens: cached,
                        });
                    }

                    // Parse finish reason
                    if let Some(finish) = chunk_json
                        .get("choices")
                        .and_then(|c| c.get(0))
                        .and_then(|c| c.get("finish_reason"))
                        .and_then(|f| f.as_str())
                    {
                        pending_finish = Some(match finish {
                            "stop" => FinishReason::Stop,
                            "tool_calls" => FinishReason::ToolCalls,
                            "length" => FinishReason::Length,
                            "content_filter" => FinishReason::ContentFilter,
                            _ => FinishReason::Stop,
                        });
                    }

                    // Emit Done when we have both finish_reason and usage
                    if pending_finish.is_some() && usage_data.is_some() {
                        yield StreamEvent::Done {
                            usage: usage_data.take(),
                            finish_reason: pending_finish.take(),
                        };
                    }
                }
            }
        })
    }

    async fn embed(
        &self,
        request: lortex_core::provider::EmbeddingRequest,
    ) -> Result<lortex_core::provider::EmbeddingResponse, ProviderError> {
        let url = format!("{}/embeddings", self.base_url);

        let mut body = serde_json::json!({
            "model": request.model,
            "input": request.input,
        });

        if let Some(fmt) = &request.encoding_format {
            body["encoding_format"] = Value::String(fmt.clone());
        }
        if let Some(dims) = request.dimensions {
            body["dimensions"] = Value::Number(dims.into());
        }

        let mut req = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json");

        if let Some(org) = &self.organization {
            req = req.header("OpenAI-Organization", org.as_str());
        }
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

        let resp_text = resp
            .text()
            .await
            .map_err(|e| ProviderError::InvalidResponse(e.to_string()))?;

        let resp_body: Value = serde_json::from_str(&resp_text).map_err(|e| {
            ProviderError::InvalidResponse(format!(
                "Failed to parse JSON: {}. Body starts with: {}",
                e,
                resp_text.chars().take(200).collect::<String>()
            ))
        })?;

        if status >= 400 {
            let message = resp_body
                .get("error")
                .and_then(|e| e.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error")
                .to_string();
            return Err(ProviderError::Api { status, message });
        }

        let data = resp_body
            .get("data")
            .and_then(|d| d.as_array())
            .ok_or_else(|| ProviderError::InvalidResponse("No data in response".into()))?;

        let embeddings: Vec<lortex_core::provider::EmbeddingData> = data
            .iter()
            .map(|item| lortex_core::provider::EmbeddingData {
                index: item.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize,
                embedding: item.get("embedding").cloned().unwrap_or(Value::Null),
            })
            .collect();

        let model = resp_body
            .get("model")
            .and_then(|m| m.as_str())
            .unwrap_or(&request.model)
            .to_string();

        let usage = resp_body.get("usage");
        let prompt_tokens = usage
            .and_then(|u| u.get("prompt_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;
        let total_tokens = usage
            .and_then(|u| u.get("total_tokens"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32;

        Ok(lortex_core::provider::EmbeddingResponse {
            data: embeddings,
            model,
            usage: lortex_core::provider::EmbeddingUsage {
                prompt_tokens,
                total_tokens,
            },
        })
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn inject_none_is_noop() {
        let mut body = json!({
            "model": "gpt-4",
            "messages": [
                {"role": "system", "content": "You are helpful"},
                {"role": "user", "content": "Hi"},
            ],
            "tools": [{"type": "function", "function": {"name": "search"}}],
        });
        let original = body.clone();
        OpenAIProvider::inject_cache_breakpoints(&mut body, CacheStrategy::None);
        assert_eq!(body, original);
    }

    #[test]
    fn inject_system_only_string_content() {
        let mut body = json!({
            "messages": [
                {"role": "system", "content": "You are helpful"},
                {"role": "user", "content": "Hi"},
            ],
        });
        OpenAIProvider::inject_cache_breakpoints(&mut body, CacheStrategy::SystemOnly);
        let sys = &body["messages"][0]["content"];
        assert!(sys.is_array());
        assert_eq!(sys[0]["cache_control"]["type"], "ephemeral");
        assert_eq!(sys[0]["text"], "You are helpful");
        // User message NOT tagged
        let user = &body["messages"][1];
        assert!(user["content"].is_string());
    }

    #[test]
    fn inject_standard_tags_tools() {
        let mut body = json!({
            "messages": [
                {"role": "system", "content": "sys"},
                {"role": "user", "content": "Hi"},
            ],
            "tools": [
                {"type": "function", "function": {"name": "a"}},
                {"type": "function", "function": {"name": "b"}},
            ],
        });
        OpenAIProvider::inject_cache_breakpoints(&mut body, CacheStrategy::Standard);
        // System tagged
        assert!(body["messages"][0]["content"][0]["cache_control"].is_object());
        // First tool NOT tagged
        assert!(body["tools"][0].get("cache_control").is_none());
        // Last tool tagged
        assert_eq!(body["tools"][1]["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn inject_full_tags_penultimate_user_string() {
        let mut body = json!({
            "messages": [
                {"role": "system", "content": "sys"},
                {"role": "user", "content": "Turn 1"},
                {"role": "assistant", "content": "Reply 1"},
                {"role": "user", "content": "Turn 2"},
            ],
        });
        OpenAIProvider::inject_cache_breakpoints(&mut body, CacheStrategy::Full);
        // Penultimate user (Turn 1) converted to blocks with cache_control
        let turn1 = &body["messages"][1]["content"];
        assert!(turn1.is_array());
        assert_eq!(turn1[0]["cache_control"]["type"], "ephemeral");
        // Last user (Turn 2) NOT tagged
        assert!(body["messages"][3]["content"].is_string());
    }

    #[test]
    fn inject_full_tags_penultimate_user_blocks() {
        let mut body = json!({
            "messages": [
                {"role": "system", "content": "sys"},
                {"role": "user", "content": [
                    {"type": "text", "text": "Part 1"},
                    {"type": "text", "text": "Part 2"},
                ]},
                {"role": "assistant", "content": "Reply"},
                {"role": "user", "content": "Turn 2"},
            ],
        });
        OpenAIProvider::inject_cache_breakpoints(&mut body, CacheStrategy::Full);
        // Last block of penultimate user message tagged
        assert!(body["messages"][1]["content"][0].get("cache_control").is_none());
        assert_eq!(body["messages"][1]["content"][1]["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn inject_preserves_existing_cache_control() {
        let mut body = json!({
            "messages": [
                {"role": "system", "content": [{"type": "text", "text": "sys", "cache_control": {"type": "custom"}}]},
                {"role": "user", "content": "Hi"},
            ],
            "tools": [{"type": "function", "function": {"name": "t"}, "cache_control": {"type": "custom"}}],
        });
        OpenAIProvider::inject_cache_breakpoints(&mut body, CacheStrategy::Standard);
        assert_eq!(body["messages"][0]["content"][0]["cache_control"]["type"], "custom");
        assert_eq!(body["tools"][0]["cache_control"]["type"], "custom");
    }
}
