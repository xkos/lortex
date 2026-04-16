//! 协议转换：OpenAI 格式 ↔ Lortex Message

use lortex_core::message::{ContentPart as LortexContent, Message, Role};
use lortex_core::provider::{CompletionRequest, CompletionResponse, ToolDefinition};
use serde_json::{json, Value};

use crate::proto::openai::{
    ChatCompletionRequest, ChatCompletionResponse, ChatMessage, Choice, ContentPart, FunctionCall,
    FunctionDef, MessageContent, Tool, ToolCall, Usage,
};

// ============================================================================
// OpenAI Request → Lortex
// ============================================================================

/// 将 OpenAI ChatMessage 转换为 Lortex Message
pub fn openai_message_to_lortex(msg: &ChatMessage) -> Message {
    let role = match msg.role.as_str() {
        "system" => Role::System,
        "user" => Role::User,
        "assistant" => Role::Assistant,
        "tool" => Role::Tool,
        _ => Role::User,
    };

    // Tool result message
    if role == Role::Tool {
        let content_text = match &msg.content {
            Some(MessageContent::Text(t)) => t.clone(),
            _ => String::new(),
        };
        let call_id = msg.tool_call_id.clone().unwrap_or_default();
        return Message::tool_result(call_id, json!(content_text), false);
    }

    // Assistant message with tool calls
    if let Some(tool_calls) = &msg.tool_calls {
        let mut parts: Vec<LortexContent> = Vec::new();

        // Add text content if present
        if let Some(content) = &msg.content {
            if let MessageContent::Text(t) = content {
                if !t.is_empty() {
                    parts.push(LortexContent::Text { text: t.clone() });
                }
            }
        }

        // Add tool calls
        for tc in tool_calls {
            let args: Value = serde_json::from_str(&tc.function.arguments)
                .unwrap_or(json!({}));
            parts.push(LortexContent::ToolCall {
                id: tc.id.clone(),
                name: tc.function.name.clone(),
                arguments: args,
            });
        }

        let mut lortex_msg = Message::assistant("");
        lortex_msg.content = parts;
        return lortex_msg;
    }

    // Regular text message (possibly multipart)
    match &msg.content {
        Some(MessageContent::Text(t)) => Message::new(role, t.clone()),
        Some(MessageContent::Parts(parts)) => {
            let mut lortex_parts = Vec::new();
            for part in parts {
                match part {
                    ContentPart::Text { text } => {
                        lortex_parts.push(LortexContent::Text { text: text.clone() });
                    }
                    ContentPart::ImageUrl { image_url } => {
                        lortex_parts.push(LortexContent::Image {
                            url: image_url.url.clone(),
                            media_type: None,
                        });
                    }
                }
            }
            let mut m = Message::new(role, "");
            m.content = lortex_parts;
            m
        }
        None => Message::new(role, ""),
    }
}

/// 将 OpenAI ChatCompletionRequest 转换为 Lortex CompletionRequest
pub fn openai_request_to_lortex(req: &ChatCompletionRequest) -> CompletionRequest {
    let messages: Vec<Message> = req.messages.iter().map(openai_message_to_lortex).collect();

    let tools: Vec<ToolDefinition> = req
        .tools
        .as_ref()
        .map(|tools| {
            tools
                .iter()
                .map(|t| ToolDefinition {
                    name: t.function.name.clone(),
                    description: t.function.description.clone().unwrap_or_default(),
                    parameters: t.function.parameters.clone().unwrap_or(json!({})),
                })
                .collect()
        })
        .unwrap_or_default();

    let stop = req.stop.as_ref().map(|s| s.to_vec()).unwrap_or_default();

    CompletionRequest {
        model: req.model.clone(),
        messages,
        tools,
        temperature: req.temperature.unwrap_or(0.7),
        max_tokens: req.max_tokens,
        stop,
        extra: Value::Null,
    }
}

// ============================================================================
// Lortex → OpenAI Response
// ============================================================================

/// 将 Lortex Message 转换为 OpenAI ChatMessage
pub fn lortex_message_to_openai(msg: &Message) -> ChatMessage {
    let role = match msg.role {
        Role::System => "system",
        Role::User => "user",
        Role::Assistant => "assistant",
        Role::Tool => "tool",
    };

    let tool_calls = msg.tool_calls();
    if !tool_calls.is_empty() {
        let oai_calls: Vec<ToolCall> = tool_calls
            .iter()
            .map(|(id, name, args)| ToolCall {
                id: id.to_string(),
                call_type: "function".into(),
                function: FunctionCall {
                    name: name.to_string(),
                    arguments: serde_json::to_string(args).unwrap_or_default(),
                },
            })
            .collect();

        let text_content = msg.text().map(|t| MessageContent::Text(t.to_string()));

        return ChatMessage {
            role: role.into(),
            content: text_content,
            name: None,
            tool_calls: Some(oai_calls),
            tool_call_id: None,
        };
    }

    // Tool result
    if msg.role == Role::Tool {
        let (call_id, content_text) = msg
            .content
            .iter()
            .find_map(|p| match p {
                LortexContent::ToolResult {
                    call_id, content, ..
                } => Some((
                    call_id.clone(),
                    content.as_str().unwrap_or("").to_string(),
                )),
                _ => None,
            })
            .unwrap_or_default();

        return ChatMessage {
            role: "tool".into(),
            content: Some(MessageContent::Text(content_text)),
            name: None,
            tool_calls: None,
            tool_call_id: Some(call_id),
        };
    }

    // Regular message
    ChatMessage {
        role: role.into(),
        content: msg.text().map(|t| MessageContent::Text(t.to_string())),
        name: None,
        tool_calls: None,
        tool_call_id: None,
    }
}

/// 将 Lortex CompletionResponse 转换为 OpenAI ChatCompletionResponse
pub fn lortex_response_to_openai(
    resp: &CompletionResponse,
    request_model: &str,
) -> ChatCompletionResponse {
    let usage = resp.usage.as_ref().map(|u| Usage {
        prompt_tokens: u.prompt_tokens,
        completion_tokens: u.completion_tokens,
        total_tokens: u.total_tokens,
        prompt_tokens_details: None,
    });

    let finish_reason = resp.finish_reason.as_ref().map(|r| {
        match r {
            lortex_core::provider::FinishReason::Stop => "stop",
            lortex_core::provider::FinishReason::ToolCalls => "tool_calls",
            lortex_core::provider::FinishReason::Length => "length",
            lortex_core::provider::FinishReason::ContentFilter => "content_filter",
        }
        .to_string()
    });

    ChatCompletionResponse {
        id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        object: "chat.completion".into(),
        created: chrono::Utc::now().timestamp(),
        model: request_model.into(),
        choices: vec![Choice {
            index: 0,
            message: lortex_message_to_openai(&resp.message),
            finish_reason,
        }],
        usage,
    }
}

/// 将 Lortex ToolDefinition 列表转换为 OpenAI Tool 列表
pub fn lortex_tools_to_openai(tools: &[ToolDefinition]) -> Vec<Tool> {
    tools
        .iter()
        .map(|t| Tool {
            tool_type: "function".into(),
            function: FunctionDef {
                name: t.name.clone(),
                description: Some(t.description.clone()),
                parameters: Some(t.parameters.clone()),
            },
        })
        .collect()
}

// ============================================================================
// Anthropic Request → Lortex
// ============================================================================

use crate::proto::anthropic::{
    AnthropicContent, ContentBlock, MessagesRequest,
    MessagesResponse, AnthropicUsage,
};

/// 将 Anthropic MessagesRequest 转换为 Lortex CompletionRequest
pub fn anthropic_request_to_lortex(req: &MessagesRequest) -> CompletionRequest {
    let mut messages = Vec::new();

    // System prompt as first message (string or content blocks → extract text)
    if let Some(system) = &req.system {
        let system_text = match system {
            Value::String(s) => s.clone(),
            Value::Array(blocks) => {
                // Extract text from content blocks
                blocks
                    .iter()
                    .filter_map(|b| b.get("text").and_then(|t| t.as_str()))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            _ => String::new(),
        };
        if !system_text.is_empty() {
            messages.push(Message::system(system_text));
        }
    }

    // Convert messages
    for msg in &req.messages {
        let role = match msg.role.as_str() {
            "user" => Role::User,
            "assistant" => Role::Assistant,
            _ => Role::User,
        };

        let mut parts = Vec::new();
        match &msg.content {
            AnthropicContent::Text(t) => {
                parts.push(LortexContent::Text { text: t.clone() });
            }
            AnthropicContent::Blocks(blocks) => {
                for block in blocks {
                    match block {
                        ContentBlock::Text { text, .. } => {
                            parts.push(LortexContent::Text { text: text.clone() });
                        }
                        ContentBlock::Image { source, .. } => {
                            let url = source.url.clone().unwrap_or_default();
                            parts.push(LortexContent::Image {
                                url,
                                media_type: source.media_type.clone(),
                            });
                        }
                        ContentBlock::ToolUse { id, name, input, .. } => {
                            parts.push(LortexContent::ToolCall {
                                id: id.clone(),
                                name: name.clone(),
                                arguments: input.clone(),
                            });
                        }
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                            ..
                        } => {
                            parts.push(LortexContent::ToolResult {
                                call_id: tool_use_id.clone(),
                                content: content.clone(),
                                is_error: *is_error,
                            });
                        }
                        // Thinking blocks 是模型内部推理，不透传给上游
                        ContentBlock::Thinking { .. } | ContentBlock::RedactedThinking { .. } => {}
                    }
                }
            }
        }

        let mut m = Message::new(role, "");
        m.content = parts;
        messages.push(m);
    }

    let tools: Vec<ToolDefinition> = req
        .tools
        .as_ref()
        .map(|tools| {
            tools
                .iter()
                .map(|t| ToolDefinition {
                    name: t.name.clone(),
                    description: t.description.clone().unwrap_or_default(),
                    parameters: t.input_schema.clone(),
                })
                .collect()
        })
        .unwrap_or_default();

    let stop = req.stop_sequences.clone().unwrap_or_default();

    CompletionRequest {
        model: req.model.clone(),
        messages,
        tools,
        temperature: req.temperature.unwrap_or(0.7),
        max_tokens: Some(req.max_tokens),
        stop,
        extra: Value::Null,
    }
}

/// 将 Lortex CompletionResponse 转换为 Anthropic MessagesResponse
pub fn lortex_response_to_anthropic(
    resp: &CompletionResponse,
    request_model: &str,
) -> MessagesResponse {
    let mut content_blocks = Vec::new();

    for part in &resp.message.content {
        match part {
            LortexContent::Text { text } => {
                content_blocks.push(ContentBlock::Text { text: text.clone(), cache_control: None });
            }
            LortexContent::ToolCall { id, name, arguments } => {
                content_blocks.push(ContentBlock::ToolUse {
                    id: id.clone(),
                    name: name.clone(),
                    input: arguments.clone(),
                    cache_control: None,
                });
            }
            _ => {}
        }
    }

    let stop_reason = resp.finish_reason.as_ref().map(|r| {
        match r {
            lortex_core::provider::FinishReason::Stop => "end_turn",
            lortex_core::provider::FinishReason::ToolCalls => "tool_use",
            lortex_core::provider::FinishReason::Length => "max_tokens",
            lortex_core::provider::FinishReason::ContentFilter => "end_turn",
        }
        .to_string()
    });

    let usage = resp.usage.as_ref().map(|u| AnthropicUsage {
        input_tokens: u.prompt_tokens,
        output_tokens: u.completion_tokens,
        cache_creation_input_tokens: None,
        cache_read_input_tokens: None,
    });

    MessagesResponse {
        id: format!("msg_{}", uuid::Uuid::new_v4()),
        response_type: "message".into(),
        role: "assistant".into(),
        content: content_blocks,
        model: request_model.into(),
        stop_reason,
        usage,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn convert_user_text_message() {
        let oai = ChatMessage {
            role: "user".into(),
            content: Some(MessageContent::Text("hello".into())),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        };
        let lortex = openai_message_to_lortex(&oai);
        assert_eq!(lortex.role, Role::User);
        assert_eq!(lortex.text(), Some("hello"));
    }

    #[test]
    fn convert_system_message() {
        let oai = ChatMessage {
            role: "system".into(),
            content: Some(MessageContent::Text("you are helpful".into())),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        };
        let lortex = openai_message_to_lortex(&oai);
        assert_eq!(lortex.role, Role::System);
    }

    #[test]
    fn convert_assistant_with_tool_calls() {
        let oai = ChatMessage {
            role: "assistant".into(),
            content: None,
            name: None,
            tool_calls: Some(vec![ToolCall {
                id: "call_1".into(),
                call_type: "function".into(),
                function: FunctionCall {
                    name: "search".into(),
                    arguments: r#"{"q":"rust"}"#.into(),
                },
            }]),
            tool_call_id: None,
        };
        let lortex = openai_message_to_lortex(&oai);
        let calls = lortex.tool_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, "search");
    }

    #[test]
    fn convert_tool_result() {
        let oai = ChatMessage {
            role: "tool".into(),
            content: Some(MessageContent::Text("found 3 results".into())),
            name: None,
            tool_calls: None,
            tool_call_id: Some("call_1".into()),
        };
        let lortex = openai_message_to_lortex(&oai);
        assert_eq!(lortex.role, Role::Tool);
    }

    #[test]
    fn convert_multipart_message() {
        let oai = ChatMessage {
            role: "user".into(),
            content: Some(MessageContent::Parts(vec![
                ContentPart::Text {
                    text: "What's this?".into(),
                },
                ContentPart::ImageUrl {
                    image_url: crate::proto::openai::ImageUrl {
                        url: "https://example.com/img.png".into(),
                        detail: None,
                    },
                },
            ])),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        };
        let lortex = openai_message_to_lortex(&oai);
        assert_eq!(lortex.content.len(), 2);
    }

    #[test]
    fn convert_request_roundtrip() {
        let oai_req = ChatCompletionRequest {
            model: "gpt-4o".into(),
            messages: vec![
                ChatMessage {
                    role: "system".into(),
                    content: Some(MessageContent::Text("be helpful".into())),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                ChatMessage {
                    role: "user".into(),
                    content: Some(MessageContent::Text("hello".into())),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
            ],
            temperature: Some(0.5),
            max_tokens: Some(1024),
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            stream: false,
            tools: None,
            tool_choice: None,
            response_format: None,
            extra: Default::default(),
        };

        let lortex = openai_request_to_lortex(&oai_req);
        assert_eq!(lortex.model, "gpt-4o");
        assert_eq!(lortex.messages.len(), 2);
        assert_eq!(lortex.temperature, 0.5);
        assert_eq!(lortex.max_tokens, Some(1024));
    }

    #[test]
    fn convert_lortex_response_to_openai() {
        let lortex_resp = CompletionResponse {
            message: Message::assistant("Hello!"),
            usage: Some(lortex_core::provider::Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
                cache_creation_input_tokens: 0,
                cache_read_input_tokens: 0,
            }),
            finish_reason: Some(lortex_core::provider::FinishReason::Stop),
            model: "gpt-4o".into(),
        };

        let oai = lortex_response_to_openai(&lortex_resp, "gpt-4o");
        assert_eq!(oai.object, "chat.completion");
        assert_eq!(oai.choices.len(), 1);
        assert_eq!(oai.choices[0].message.role, "assistant");
        assert_eq!(oai.choices[0].finish_reason.as_deref(), Some("stop"));
        assert_eq!(oai.usage.as_ref().unwrap().total_tokens, 15);
    }

    #[test]
    fn convert_lortex_tool_call_response() {
        let mut msg = Message::assistant("");
        msg.content = vec![LortexContent::ToolCall {
            id: "call_1".into(),
            name: "search".into(),
            arguments: json!({"q": "rust"}),
        }];

        let oai = lortex_message_to_openai(&msg);
        let calls = oai.tool_calls.unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].function.name, "search");
        assert!(calls[0].function.arguments.contains("rust"));
    }

    #[test]
    fn convert_request_with_tools() {
        let oai_req = ChatCompletionRequest {
            model: "gpt-4o".into(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: Some(MessageContent::Text("search".into())),
                name: None,
                tool_calls: None,
                tool_call_id: None,
            }],
            temperature: None,
            max_tokens: None,
            top_p: None,
            frequency_penalty: None,
            presence_penalty: None,
            stop: None,
            stream: false,
            tools: Some(vec![Tool {
                tool_type: "function".into(),
                function: FunctionDef {
                    name: "search".into(),
                    description: Some("Search".into()),
                    parameters: Some(json!({"type": "object"})),
                },
            }]),
            tool_choice: None,
            response_format: None,
            extra: Default::default(),
        };

        let lortex = openai_request_to_lortex(&oai_req);
        assert_eq!(lortex.tools.len(), 1);
        assert_eq!(lortex.tools[0].name, "search");
    }
}
