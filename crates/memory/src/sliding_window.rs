//! Sliding window memory — keeps only the most recent N messages.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use lortex_core::error::MemoryError;
use lortex_core::memory::{Memory, MemoryEntry, RetrieveOptions, SearchOptions};
use lortex_core::message::Message;

/// Memory that keeps only the most recent `window_size` messages per session.
pub struct SlidingWindowMemory {
    window_size: usize,
    sessions: Arc<RwLock<HashMap<String, Vec<Message>>>>,
}

impl SlidingWindowMemory {
    pub fn new(window_size: usize) -> Self {
        Self {
            window_size,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl Memory for SlidingWindowMemory {
    async fn store_messages(
        &self,
        session_id: &str,
        messages: &[Message],
    ) -> Result<(), MemoryError> {
        let mut sessions = self.sessions.write().await;
        let entry = sessions.entry(session_id.to_string()).or_default();
        entry.extend(messages.iter().cloned());

        // Trim to window size
        if entry.len() > self.window_size {
            let drain_count = entry.len() - self.window_size;
            entry.drain(..drain_count);
        }

        Ok(())
    }

    async fn get_messages(
        &self,
        session_id: &str,
        opts: RetrieveOptions,
    ) -> Result<Vec<Message>, MemoryError> {
        let sessions = self.sessions.read().await;
        let messages = sessions.get(session_id).cloned().unwrap_or_default();

        let offset = opts.offset.unwrap_or(0);
        let limit = opts.limit.unwrap_or(messages.len());

        Ok(messages.into_iter().skip(offset).take(limit).collect())
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        let sessions = self.sessions.read().await;
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        let iter: Box<dyn Iterator<Item = (&String, &Vec<Message>)>> =
            if let Some(sid) = &opts.session_id {
                if let Some(msgs) = sessions.get(sid) {
                    Box::new(std::iter::once((sid, msgs)))
                } else {
                    Box::new(std::iter::empty())
                }
            } else {
                Box::new(sessions.iter())
            };

        for (_sid, messages) in iter {
            for msg in messages {
                if let Some(text) = msg.text() {
                    if text.to_lowercase().contains(&query_lower) {
                        results.push(MemoryEntry {
                            message: msg.clone(),
                            score: Some(1.0),
                            metadata: Default::default(),
                        });
                        if results.len() >= opts.limit {
                            return Ok(results);
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    async fn clear(&self, session_id: &str) -> Result<(), MemoryError> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lortex_core::memory::{RetrieveOptions, SearchOptions};
    use lortex_core::message::Message;

    #[tokio::test]
    async fn store_within_window() {
        let mem = SlidingWindowMemory::new(5);
        let msgs: Vec<Message> = (0..3).map(|i| Message::user(format!("msg{i}"))).collect();
        mem.store_messages("s1", &msgs).await.unwrap();

        let retrieved = mem
            .get_messages("s1", RetrieveOptions::default())
            .await
            .unwrap();
        assert_eq!(retrieved.len(), 3);
    }

    #[tokio::test]
    async fn trims_to_window_size() {
        let mem = SlidingWindowMemory::new(3);
        let msgs: Vec<Message> = (0..5).map(|i| Message::user(format!("msg{i}"))).collect();
        mem.store_messages("s1", &msgs).await.unwrap();

        let retrieved = mem
            .get_messages("s1", RetrieveOptions::default())
            .await
            .unwrap();
        assert_eq!(retrieved.len(), 3);
        // Should keep the last 3 messages
        assert_eq!(retrieved[0].text(), Some("msg2"));
        assert_eq!(retrieved[1].text(), Some("msg3"));
        assert_eq!(retrieved[2].text(), Some("msg4"));
    }

    #[tokio::test]
    async fn incremental_store_trims() {
        let mem = SlidingWindowMemory::new(3);
        mem.store_messages("s1", &[Message::user("a"), Message::user("b")])
            .await
            .unwrap();
        mem.store_messages("s1", &[Message::user("c"), Message::user("d")])
            .await
            .unwrap();

        let retrieved = mem
            .get_messages("s1", RetrieveOptions::default())
            .await
            .unwrap();
        assert_eq!(retrieved.len(), 3);
        assert_eq!(retrieved[0].text(), Some("b"));
        assert_eq!(retrieved[1].text(), Some("c"));
        assert_eq!(retrieved[2].text(), Some("d"));
    }

    #[tokio::test]
    async fn get_messages_with_limit_and_offset() {
        let mem = SlidingWindowMemory::new(10);
        let msgs: Vec<Message> = (0..5).map(|i| Message::user(format!("msg{i}"))).collect();
        mem.store_messages("s1", &msgs).await.unwrap();

        let opts = RetrieveOptions {
            limit: Some(2),
            offset: Some(1),
            after: None,
        };
        let retrieved = mem.get_messages("s1", opts).await.unwrap();
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved[0].text(), Some("msg1"));
        assert_eq!(retrieved[1].text(), Some("msg2"));
    }

    #[tokio::test]
    async fn search_within_window() {
        let mem = SlidingWindowMemory::new(3);
        let msgs: Vec<Message> = (0..5)
            .map(|i| Message::user(format!("item {i}")))
            .collect();
        mem.store_messages("s1", &msgs).await.unwrap();

        // Only the last 3 messages should be searchable
        let results = mem.search("item", SearchOptions::default()).await.unwrap();
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn search_case_insensitive() {
        let mem = SlidingWindowMemory::new(10);
        mem.store_messages("s1", &[Message::user("Hello WORLD")])
            .await
            .unwrap();

        let results = mem
            .search("hello world", SearchOptions::default())
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn search_filters_by_session() {
        let mem = SlidingWindowMemory::new(10);
        mem.store_messages("s1", &[Message::user("target")])
            .await
            .unwrap();
        mem.store_messages("s2", &[Message::user("target")])
            .await
            .unwrap();

        let opts = SearchOptions {
            session_id: Some("s1".into()),
            ..Default::default()
        };
        let results = mem.search("target", opts).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn clear_removes_session() {
        let mem = SlidingWindowMemory::new(10);
        mem.store_messages("s1", &[Message::user("hello")])
            .await
            .unwrap();
        mem.clear("s1").await.unwrap();

        let retrieved = mem
            .get_messages("s1", RetrieveOptions::default())
            .await
            .unwrap();
        assert!(retrieved.is_empty());
    }

    #[tokio::test]
    async fn window_size_one() {
        let mem = SlidingWindowMemory::new(1);
        mem.store_messages("s1", &[Message::user("a"), Message::user("b")])
            .await
            .unwrap();

        let retrieved = mem
            .get_messages("s1", RetrieveOptions::default())
            .await
            .unwrap();
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].text(), Some("b"));
    }

    #[tokio::test]
    async fn empty_session_returns_empty() {
        let mem = SlidingWindowMemory::new(5);
        let retrieved = mem
            .get_messages("nonexistent", RetrieveOptions::default())
            .await
            .unwrap();
        assert!(retrieved.is_empty());
    }
}
