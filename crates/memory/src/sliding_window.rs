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
