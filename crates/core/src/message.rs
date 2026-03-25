//! 统一消息格式 — Agent 框架中所有通信的基础数据结构
//!
//! 消息支持多部分内容（文本、图片、工具调用、工具结果），
//! 通过 [`Role`] 区分发送者身份（System/User/Assistant/Tool）。

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

/// The role of a message participant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// A single part of message content.
/// Messages can contain multiple content parts (text, images, tool calls, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    /// Plain text content.
    Text { text: String },

    /// Image content, referenced by URL or base64.
    Image {
        url: String,
        media_type: Option<String>,
    },

    /// A tool call request from the assistant.
    ToolCall {
        id: String,
        name: String,
        arguments: Value,
    },

    /// A tool result returned after execution.
    ToolResult {
        call_id: String,
        content: Value,
        is_error: bool,
    },
}

/// A message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique identifier for the message.
    pub id: String,

    /// Role of the message sender.
    pub role: Role,

    /// Content parts of the message.
    pub content: Vec<ContentPart>,

    /// Arbitrary metadata attached to the message.
    #[serde(default)]
    pub metadata: HashMap<String, Value>,

    /// Timestamp when the message was created.
    pub timestamp: DateTime<Utc>,
}

impl Message {
    /// Create a new text message with the given role.
    pub fn new(role: Role, text: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role,
            content: vec![ContentPart::Text {
                text: text.into(),
            }],
            metadata: HashMap::new(),
            timestamp: Utc::now(),
        }
    }

    /// Create a system message.
    pub fn system(text: impl Into<String>) -> Self {
        Self::new(Role::System, text)
    }

    /// Create a user message.
    pub fn user(text: impl Into<String>) -> Self {
        Self::new(Role::User, text)
    }

    /// Create an assistant message.
    pub fn assistant(text: impl Into<String>) -> Self {
        Self::new(Role::Assistant, text)
    }

    /// Create a tool result message.
    pub fn tool_result(call_id: impl Into<String>, content: Value, is_error: bool) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            role: Role::Tool,
            content: vec![ContentPart::ToolResult {
                call_id: call_id.into(),
                content,
                is_error,
            }],
            metadata: HashMap::new(),
            timestamp: Utc::now(),
        }
    }

    /// Get the first text content from the message, if any.
    pub fn text(&self) -> Option<&str> {
        self.content.iter().find_map(|part| match part {
            ContentPart::Text { text } => Some(text.as_str()),
            _ => None,
        })
    }

    /// Get all tool calls from the message.
    pub fn tool_calls(&self) -> Vec<(&str, &str, &Value)> {
        self.content
            .iter()
            .filter_map(|part| match part {
                ContentPart::ToolCall {
                    id,
                    name,
                    arguments,
                } => Some((id.as_str(), name.as_str(), arguments)),
                _ => None,
            })
            .collect()
    }

    /// Add metadata to the message.
    pub fn with_metadata(mut self, key: impl Into<String>, value: Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn role_serde_roundtrip() {
        for (role, expected) in [
            (Role::System, "\"system\""),
            (Role::User, "\"user\""),
            (Role::Assistant, "\"assistant\""),
            (Role::Tool, "\"tool\""),
        ] {
            let json = serde_json::to_string(&role).unwrap();
            assert_eq!(json, expected);
            let back: Role = serde_json::from_str(&json).unwrap();
            assert_eq!(back, role);
        }
    }

    #[test]
    fn content_part_text_serde() {
        let part = ContentPart::Text {
            text: "hello".into(),
        };
        let json = serde_json::to_string(&part).unwrap();
        assert!(json.contains("\"type\":\"text\""));
        let back: ContentPart = serde_json::from_str(&json).unwrap();
        match back {
            ContentPart::Text { text } => assert_eq!(text, "hello"),
            _ => panic!("expected Text variant"),
        }
    }

    #[test]
    fn content_part_image_serde() {
        let part = ContentPart::Image {
            url: "https://example.com/img.png".into(),
            media_type: Some("image/png".into()),
        };
        let json = serde_json::to_string(&part).unwrap();
        let back: ContentPart = serde_json::from_str(&json).unwrap();
        match back {
            ContentPart::Image { url, media_type } => {
                assert_eq!(url, "https://example.com/img.png");
                assert_eq!(media_type.as_deref(), Some("image/png"));
            }
            _ => panic!("expected Image variant"),
        }
    }

    #[test]
    fn content_part_tool_call_serde() {
        let part = ContentPart::ToolCall {
            id: "call_1".into(),
            name: "search".into(),
            arguments: json!({"query": "rust"}),
        };
        let json = serde_json::to_string(&part).unwrap();
        assert!(json.contains("\"type\":\"tool_call\""));
        let back: ContentPart = serde_json::from_str(&json).unwrap();
        match back {
            ContentPart::ToolCall {
                id,
                name,
                arguments,
            } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "search");
                assert_eq!(arguments, json!({"query": "rust"}));
            }
            _ => panic!("expected ToolCall variant"),
        }
    }

    #[test]
    fn content_part_tool_result_serde() {
        let part = ContentPart::ToolResult {
            call_id: "call_1".into(),
            content: json!({"result": 42}),
            is_error: false,
        };
        let json = serde_json::to_string(&part).unwrap();
        let back: ContentPart = serde_json::from_str(&json).unwrap();
        match back {
            ContentPart::ToolResult {
                call_id,
                content,
                is_error,
            } => {
                assert_eq!(call_id, "call_1");
                assert_eq!(content, json!({"result": 42}));
                assert!(!is_error);
            }
            _ => panic!("expected ToolResult variant"),
        }
    }

    #[test]
    fn message_serde_roundtrip() {
        let msg = Message::user("hello world");
        let json = serde_json::to_string(&msg).unwrap();
        let back: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(back.role, Role::User);
        assert_eq!(back.text(), Some("hello world"));
        assert_eq!(back.id, msg.id);
    }

    #[test]
    fn message_convenience_constructors() {
        let sys = Message::system("you are helpful");
        assert_eq!(sys.role, Role::System);
        assert_eq!(sys.text(), Some("you are helpful"));

        let user = Message::user("hi");
        assert_eq!(user.role, Role::User);

        let asst = Message::assistant("hello");
        assert_eq!(asst.role, Role::Assistant);

        let tool = Message::tool_result("call_1", json!("done"), false);
        assert_eq!(tool.role, Role::Tool);
        match &tool.content[0] {
            ContentPart::ToolResult {
                call_id, is_error, ..
            } => {
                assert_eq!(call_id, "call_1");
                assert!(!is_error);
            }
            _ => panic!("expected ToolResult"),
        }
    }

    #[test]
    fn message_empty_content() {
        let msg = Message {
            id: "test".into(),
            role: Role::User,
            content: vec![],
            metadata: HashMap::new(),
            timestamp: Utc::now(),
        };
        assert_eq!(msg.text(), None);
        assert!(msg.tool_calls().is_empty());
        let json = serde_json::to_string(&msg).unwrap();
        let back: Message = serde_json::from_str(&json).unwrap();
        assert!(back.content.is_empty());
    }

    #[test]
    fn message_multi_part() {
        let msg = Message {
            id: "multi".into(),
            role: Role::Assistant,
            content: vec![
                ContentPart::Text {
                    text: "Let me search".into(),
                },
                ContentPart::ToolCall {
                    id: "c1".into(),
                    name: "search".into(),
                    arguments: json!({"q": "test"}),
                },
                ContentPart::ToolCall {
                    id: "c2".into(),
                    name: "read".into(),
                    arguments: json!({"path": "/tmp"}),
                },
            ],
            metadata: HashMap::new(),
            timestamp: Utc::now(),
        };
        assert_eq!(msg.text(), Some("Let me search"));
        let calls = msg.tool_calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].1, "search");
        assert_eq!(calls[1].1, "read");
    }

    #[test]
    fn message_with_metadata() {
        let msg = Message::user("test")
            .with_metadata("source", json!("api"))
            .with_metadata("priority", json!(1));
        assert_eq!(msg.metadata.len(), 2);
        assert_eq!(msg.metadata["source"], json!("api"));
    }

    #[test]
    fn message_metadata_default_on_deserialize() {
        let json = r#"{"id":"x","role":"user","content":[{"type":"text","text":"hi"}],"timestamp":"2025-01-01T00:00:00Z"}"#;
        let msg: Message = serde_json::from_str(json).unwrap();
        assert!(msg.metadata.is_empty());
    }
}
