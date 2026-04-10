//! In-memory conversation store.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use lortex_core::error::MemoryError;
use lortex_core::memory::{Memory, MemoryEntry, RetrieveOptions, SearchOptions};
use lortex_core::message::Message;

/// A simple in-memory store that keeps all messages per session.
pub struct InMemoryStore {
    sessions: Arc<RwLock<HashMap<String, Vec<Message>>>>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Memory for InMemoryStore {
    async fn store_messages(
        &self,
        session_id: &str,
        messages: &[Message],
    ) -> Result<(), MemoryError> {
        let mut sessions = self.sessions.write().await;
        let entry = sessions.entry(session_id.to_string()).or_default();
        entry.extend(messages.iter().cloned());
        tracing::debug!(
            session_id = session_id,
            count = messages.len(),
            "Stored messages in memory"
        );
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

        let filtered: Vec<Message> = messages
            .into_iter()
            .filter(|m| {
                if let Some(after) = &opts.after {
                    m.timestamp >= *after
                } else {
                    true
                }
            })
            .skip(offset)
            .take(limit)
            .collect();

        Ok(filtered)
    }

    async fn search(
        &self,
        query: &str,
        opts: SearchOptions,
    ) -> Result<Vec<MemoryEntry>, MemoryError> {
        let sessions = self.sessions.read().await;

        // Simple text-based search (not semantic — for semantic search,
        // use a vector store implementation).
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
        tracing::debug!(session_id = session_id, "Cleared session from memory");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lortex_core::memory::{RetrieveOptions, SearchOptions};
    use lortex_core::message::Message;

    #[tokio::test]
    async fn store_and_get_messages() {
        let store = InMemoryStore::new();
        let msgs = vec![Message::user("hello"), Message::assistant("hi")];
        store.store_messages("s1", &msgs).await.unwrap();

        let retrieved = store
            .get_messages("s1", RetrieveOptions::default())
            .await
            .unwrap();
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved[0].text(), Some("hello"));
        assert_eq!(retrieved[1].text(), Some("hi"));
    }

    #[tokio::test]
    async fn get_messages_empty_session() {
        let store = InMemoryStore::new();
        let retrieved = store
            .get_messages("nonexistent", RetrieveOptions::default())
            .await
            .unwrap();
        assert!(retrieved.is_empty());
    }

    #[tokio::test]
    async fn get_messages_with_limit_and_offset() {
        let store = InMemoryStore::new();
        let msgs: Vec<Message> = (0..5).map(|i| Message::user(format!("msg{i}"))).collect();
        store.store_messages("s1", &msgs).await.unwrap();

        let opts = RetrieveOptions {
            limit: Some(2),
            offset: Some(1),
            after: None,
        };
        let retrieved = store.get_messages("s1", opts).await.unwrap();
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved[0].text(), Some("msg1"));
        assert_eq!(retrieved[1].text(), Some("msg2"));
    }

    #[tokio::test]
    async fn get_messages_with_after_filter() {
        let store = InMemoryStore::new();
        let early = Message::user("early");
        let early_ts = early.timestamp;
        store.store_messages("s1", &[early]).await.unwrap();

        // Store a later message
        let late = Message::user("late");
        let late_ts = late.timestamp;
        store.store_messages("s1", &[late]).await.unwrap();

        let opts = RetrieveOptions {
            after: Some(late_ts),
            ..Default::default()
        };
        let retrieved = store.get_messages("s1", opts).await.unwrap();
        // Should include messages with timestamp >= late_ts
        assert!(retrieved.iter().all(|m| m.timestamp >= late_ts));
        // early message should be filtered out if its timestamp < late_ts
        assert!(retrieved.iter().all(|m| m.text() != Some("early") || early_ts >= late_ts));
    }

    #[tokio::test]
    async fn store_appends_to_existing_session() {
        let store = InMemoryStore::new();
        store
            .store_messages("s1", &[Message::user("first")])
            .await
            .unwrap();
        store
            .store_messages("s1", &[Message::user("second")])
            .await
            .unwrap();

        let retrieved = store
            .get_messages("s1", RetrieveOptions::default())
            .await
            .unwrap();
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved[0].text(), Some("first"));
        assert_eq!(retrieved[1].text(), Some("second"));
    }

    #[tokio::test]
    async fn search_finds_matching_text() {
        let store = InMemoryStore::new();
        store
            .store_messages(
                "s1",
                &[
                    Message::user("hello world"),
                    Message::user("goodbye world"),
                    Message::user("nothing here"),
                ],
            )
            .await
            .unwrap();

        let results = store
            .search("world", SearchOptions::default())
            .await
            .unwrap();
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn search_case_insensitive() {
        let store = InMemoryStore::new();
        store
            .store_messages("s1", &[Message::user("Hello WORLD")])
            .await
            .unwrap();

        let results = store
            .search("hello world", SearchOptions::default())
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn search_respects_limit() {
        let store = InMemoryStore::new();
        let msgs: Vec<Message> = (0..10)
            .map(|i| Message::user(format!("match {i}")))
            .collect();
        store.store_messages("s1", &msgs).await.unwrap();

        let opts = SearchOptions {
            limit: 3,
            ..Default::default()
        };
        let results = store.search("match", opts).await.unwrap();
        assert_eq!(results.len(), 3);
    }

    #[tokio::test]
    async fn search_filters_by_session_id() {
        let store = InMemoryStore::new();
        store
            .store_messages("s1", &[Message::user("target text")])
            .await
            .unwrap();
        store
            .store_messages("s2", &[Message::user("target text")])
            .await
            .unwrap();

        let opts = SearchOptions {
            session_id: Some("s1".into()),
            ..Default::default()
        };
        let results = store.search("target", opts).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    async fn search_no_results() {
        let store = InMemoryStore::new();
        store
            .store_messages("s1", &[Message::user("hello")])
            .await
            .unwrap();

        let results = store
            .search("nonexistent", SearchOptions::default())
            .await
            .unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn clear_removes_session() {
        let store = InMemoryStore::new();
        store
            .store_messages("s1", &[Message::user("hello")])
            .await
            .unwrap();
        store.clear("s1").await.unwrap();

        let retrieved = store
            .get_messages("s1", RetrieveOptions::default())
            .await
            .unwrap();
        assert!(retrieved.is_empty());
    }

    #[tokio::test]
    async fn clear_nonexistent_session_is_ok() {
        let store = InMemoryStore::new();
        // Should not error
        store.clear("nonexistent").await.unwrap();
    }

    #[tokio::test]
    async fn multiple_sessions_isolated() {
        let store = InMemoryStore::new();
        store
            .store_messages("s1", &[Message::user("session1")])
            .await
            .unwrap();
        store
            .store_messages("s2", &[Message::user("session2")])
            .await
            .unwrap();

        let r1 = store
            .get_messages("s1", RetrieveOptions::default())
            .await
            .unwrap();
        let r2 = store
            .get_messages("s2", RetrieveOptions::default())
            .await
            .unwrap();
        assert_eq!(r1.len(), 1);
        assert_eq!(r1[0].text(), Some("session1"));
        assert_eq!(r2.len(), 1);
        assert_eq!(r2[0].text(), Some("session2"));
    }

    #[test]
    fn default_impl() {
        let _store = InMemoryStore::default();
    }
}
