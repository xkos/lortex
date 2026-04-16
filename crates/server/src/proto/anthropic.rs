//! Anthropic Messages API 兼容协议类型

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ============================================================================
// Messages Request
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagesRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessage>,
    pub max_tokens: u32,
    /// System prompt — 支持 string 或 content block 数组（含 cache_control）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<AnthropicTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    /// 透传的额外字段
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicMessage {
    pub role: String,
    pub content: AnthropicContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AnthropicContent {
    Text(String),
    Blocks(Vec<ContentBlock>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text {
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<Value>,
    },
    #[serde(rename = "image")]
    Image {
        source: ImageSource,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<Value>,
    },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<Value>,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: Value,
        #[serde(default)]
        is_error: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<Value>,
    },
    /// Extended thinking block — 原样保留，转换时忽略
    #[serde(rename = "thinking")]
    Thinking {
        thinking: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<Value>,
    },
    /// 被裁减的 thinking block（长对话时 Anthropic 会压缩）
    #[serde(rename = "redacted_thinking")]
    RedactedThinking {
        data: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        cache_control: Option<Value>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageSource {
    #[serde(rename = "type")]
    pub source_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicTool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: Value,
}

// ============================================================================
// Messages Response
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagesResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub response_type: String,
    pub role: String,
    pub content: Vec<ContentBlock>,
    pub model: String,
    pub stop_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<AnthropicUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_creation_input_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_input_tokens: Option<u32>,
}

// ============================================================================
// Error
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub error: AnthropicErrorDetail,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicErrorDetail {
    #[serde(rename = "type")]
    pub detail_type: String,
    pub message: String,
}

impl AnthropicError {
    pub fn new(error_type: &str, detail_type: &str, message: impl Into<String>) -> Self {
        Self {
            error_type: error_type.into(),
            error: AnthropicErrorDetail {
                detail_type: detail_type.into(),
                message: message.into(),
            },
        }
    }

    pub fn invalid_request(message: impl Into<String>) -> Self {
        Self::new("error", "invalid_request_error", message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new("error", "not_found_error", message)
    }

    pub fn auth_error(message: impl Into<String>) -> Self {
        Self::new("error", "authentication_error", message)
    }
}

// ============================================================================
// Streaming SSE Events
// ============================================================================

/// message_start event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageStartEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub message: MessageStartData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageStartData {
    pub id: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub role: String,
    pub content: Vec<ContentBlock>,
    pub model: String,
    pub stop_reason: Option<String>,
    pub usage: AnthropicUsage,
}

/// content_block_start event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentBlockStartEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub index: usize,
    pub content_block: ContentBlock,
}

/// content_block_delta event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentBlockDeltaEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub index: usize,
    pub delta: DeltaBlock,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DeltaBlock {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
}

/// content_block_stop event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentBlockStopEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub index: usize,
}

/// message_delta event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDeltaEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub delta: MessageDelta,
    pub usage: MessageDeltaUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDelta {
    pub stop_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageDeltaUsage {
    pub output_tokens: u32,
}

/// message_stop event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageStopEvent {
    #[serde(rename = "type")]
    pub event_type: String,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn messages_request_minimal() {
        let json = json!({
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "Hello"}]
        });
        let req: MessagesRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.model, "claude-sonnet-4-20250514");
        assert_eq!(req.max_tokens, 1024);
        assert!(!req.stream);
    }

    #[test]
    fn messages_request_with_system() {
        let json = json!({
            "model": "claude-sonnet-4-20250514",
            "max_tokens": 1024,
            "system": "You are helpful",
            "messages": [{"role": "user", "content": "Hi"}]
        });
        let req: MessagesRequest = serde_json::from_value(json).unwrap();
        assert_eq!(req.system.as_ref().and_then(|v| v.as_str()), Some("You are helpful"));
    }

    #[test]
    fn content_block_text() {
        let json = json!({"role": "user", "content": [{"type": "text", "text": "hello"}]});
        let msg: AnthropicMessage = serde_json::from_value(json).unwrap();
        match msg.content {
            AnthropicContent::Blocks(blocks) => {
                assert_eq!(blocks.len(), 1);
                match &blocks[0] {
                    ContentBlock::Text { text, .. } => assert_eq!(text, "hello"),
                    _ => panic!("expected text block"),
                }
            }
            _ => panic!("expected blocks"),
        }
    }

    #[test]
    fn content_string_shorthand() {
        let json = json!({"role": "user", "content": "hello"});
        let msg: AnthropicMessage = serde_json::from_value(json).unwrap();
        match msg.content {
            AnthropicContent::Text(t) => assert_eq!(t, "hello"),
            _ => panic!("expected text"),
        }
    }

    #[test]
    fn tool_use_block() {
        let json = json!({
            "type": "tool_use",
            "id": "toolu_123",
            "name": "search",
            "input": {"query": "rust"}
        });
        let block: ContentBlock = serde_json::from_value(json).unwrap();
        match block {
            ContentBlock::ToolUse { id, name, input, .. } => {
                assert_eq!(id, "toolu_123");
                assert_eq!(name, "search");
                assert_eq!(input["query"], "rust");
            }
            _ => panic!("expected tool_use"),
        }
    }

    #[test]
    fn messages_response_serialize() {
        let resp = MessagesResponse {
            id: "msg_123".into(),
            response_type: "message".into(),
            role: "assistant".into(),
            content: vec![ContentBlock::Text {
                text: "Hello!".into(),
                cache_control: None,
            }],
            model: "claude-sonnet-4-20250514".into(),
            stop_reason: Some("end_turn".into()),
            usage: Some(AnthropicUsage {
                input_tokens: 10,
                output_tokens: 5,
                cache_creation_input_tokens: None,
                cache_read_input_tokens: None,
            }),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("msg_123"));
        assert!(json.contains("Hello!"));
    }

    #[test]
    fn error_serialize() {
        let err = AnthropicError::invalid_request("model not found");
        let json = serde_json::to_string(&err).unwrap();
        assert!(json.contains("invalid_request_error"));
    }

    #[test]
    fn message_start_event_serialize() {
        let event = MessageStartEvent {
            event_type: "message_start".into(),
            message: MessageStartData {
                id: "msg_123".into(),
                msg_type: "message".into(),
                role: "assistant".into(),
                content: vec![],
                model: "claude-sonnet-4-20250514".into(),
                stop_reason: None,
                usage: AnthropicUsage {
                    input_tokens: 10,
                    output_tokens: 0,
                    cache_creation_input_tokens: None,
                    cache_read_input_tokens: None,
                },
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("message_start"));
        assert!(json.contains("msg_123"));
    }

    #[test]
    fn content_block_delta_text() {
        let event = ContentBlockDeltaEvent {
            event_type: "content_block_delta".into(),
            index: 0,
            delta: DeltaBlock::TextDelta { text: "Hello".into() },
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("text_delta"));
        assert!(json.contains("Hello"));
    }

    #[test]
    fn content_block_delta_tool() {
        let event = ContentBlockDeltaEvent {
            event_type: "content_block_delta".into(),
            index: 1,
            delta: DeltaBlock::InputJsonDelta { partial_json: r#"{"q":"#.into() },
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("input_json_delta"));
    }

    #[test]
    fn message_delta_event_serialize() {
        let event = MessageDeltaEvent {
            event_type: "message_delta".into(),
            delta: MessageDelta { stop_reason: "end_turn".into() },
            usage: MessageDeltaUsage { output_tokens: 42 },
        };
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("end_turn"));
        assert!(json.contains("42"));
    }
}
