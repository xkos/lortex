//! Memory trait and types — short-term and long-term memory abstractions.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

use crate::error::MemoryError;
use crate::message::Message;

/// Options for retrieving messages from memory.
#[derive(Debug, Clone, Default)]
pub struct RetrieveOptions {
    /// Maximum number of messages to return.
    pub limit: Option<usize>,

    /// Skip the first N messages.
    pub offset: Option<usize>,

    /// Only return messages after this timestamp (inclusive).
    pub after: Option<chrono::DateTime<chrono::Utc>>,
}

/// Options for semantic search in memory.
#[derive(Debug, Clone)]
pub struct SearchOptions {
    /// Maximum number of results.
    pub limit: usize,

    /// Minimum similarity score (0.0 - 1.0).
    pub min_score: f32,

    /// Filter by session ID.
    pub session_id: Option<String>,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            limit: 10,
            min_score: 0.0,
            session_id: None,
        }
    }
}

/// A single memory entry returned from search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    /// The stored message.
    pub message: Message,

    /// Similarity score (if from a search).
    pub score: Option<f32>,

    /// Arbitrary metadata.
    #[serde(default)]
    pub metadata: std::collections::HashMap<String, Value>,
}

/// The core Memory trait. Provides storage and retrieval of conversation messages.
#[async_trait]
pub trait Memory: Send + Sync {
    /// Store messages for a session.
    async fn store_messages(
        &self,
        session_id: &str,
        messages: &[Message],
    ) -> Result<(), MemoryError>;

    /// Retrieve messages for a session.
    async fn get_messages(
        &self,
        session_id: &str,
        opts: RetrieveOptions,
    ) -> Result<Vec<Message>, MemoryError>;

    /// Semantic search across stored messages.
    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<MemoryEntry>, MemoryError>;

    /// Clear all messages for a session.
    async fn clear(&self, session_id: &str) -> Result<(), MemoryError>;
}

impl fmt::Debug for dyn Memory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Memory").finish()
    }
}

/// A layered memory system that composes working, short-term, and long-term memory.
pub struct LayeredMemory {
    /// Working memory: current task context (ephemeral).
    pub working: Box<dyn Memory>,

    /// Short-term memory: conversation history for the current session.
    pub short_term: Box<dyn Memory>,

    /// Long-term memory: persistent memory across sessions.
    pub long_term: Box<dyn Memory>,
}

impl LayeredMemory {
    pub fn new(
        working: Box<dyn Memory>,
        short_term: Box<dyn Memory>,
        long_term: Box<dyn Memory>,
    ) -> Self {
        Self {
            working,
            short_term,
            long_term,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn retrieve_options_default() {
        let opts = RetrieveOptions::default();
        assert!(opts.limit.is_none());
        assert!(opts.offset.is_none());
        assert!(opts.after.is_none());
    }

    #[test]
    fn search_options_default() {
        let opts = SearchOptions::default();
        assert_eq!(opts.limit, 10);
        assert_eq!(opts.min_score, 0.0);
        assert!(opts.session_id.is_none());
    }

    #[test]
    fn memory_entry_serde_roundtrip() {
        let entry = MemoryEntry {
            message: Message::user("test"),
            score: Some(0.95),
            metadata: {
                let mut m = std::collections::HashMap::new();
                m.insert("source".into(), json!("search"));
                m
            },
        };
        let json_str = serde_json::to_string(&entry).unwrap();
        let back: MemoryEntry = serde_json::from_str(&json_str).unwrap();
        assert_eq!(back.score, Some(0.95));
        assert_eq!(back.metadata["source"], json!("search"));
        assert_eq!(back.message.text(), Some("test"));
    }

    #[test]
    fn memory_entry_without_score() {
        let entry = MemoryEntry {
            message: Message::user("hi"),
            score: None,
            metadata: std::collections::HashMap::new(),
        };
        let json_str = serde_json::to_string(&entry).unwrap();
        let back: MemoryEntry = serde_json::from_str(&json_str).unwrap();
        assert!(back.score.is_none());
        assert!(back.metadata.is_empty());
    }
}
