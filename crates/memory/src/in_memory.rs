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
