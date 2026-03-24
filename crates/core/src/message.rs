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
