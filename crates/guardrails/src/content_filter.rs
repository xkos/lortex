//! Content filter guardrail — blocks messages containing specified patterns.

use async_trait::async_trait;

use lortex_core::guardrail::{Guardrail, GuardrailResult};
use lortex_core::message::Message;

/// A guardrail that filters messages based on blocked word patterns.
pub struct ContentFilter {
    /// Blocked words/phrases (case-insensitive matching).
    blocked_patterns: Vec<String>,
}

impl ContentFilter {
    /// Create a new content filter with the given blocked patterns.
    pub fn new(patterns: Vec<String>) -> Self {
        Self {
            blocked_patterns: patterns
                .into_iter()
                .map(|p| p.to_lowercase())
                .collect(),
        }
    }

    /// Create a content filter with default blocked patterns.
    pub fn default_filter() -> Self {
        Self::new(vec![
            // Add default patterns as needed
        ])
    }

    /// Add a blocked pattern.
    pub fn add_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.blocked_patterns.push(pattern.into().to_lowercase());
        self
    }

    fn check_text(&self, text: &str) -> GuardrailResult {
        let lower = text.to_lowercase();
        for pattern in &self.blocked_patterns {
            if lower.contains(pattern) {
                return GuardrailResult::Block {
                    message: format!("Content blocked: contains prohibited pattern '{}'", pattern),
                };
            }
        }
        GuardrailResult::Pass
    }
}

#[async_trait]
impl Guardrail for ContentFilter {
    fn name(&self) -> &str {
        "content_filter"
    }

    async fn check_input(&self, messages: &[Message]) -> GuardrailResult {
        for msg in messages {
            if let Some(text) = msg.text() {
                let result = self.check_text(text);
                if result.is_block() {
                    return result;
                }
            }
        }
        GuardrailResult::Pass
    }

    async fn check_output(&self, output: &Message) -> GuardrailResult {
        if let Some(text) = output.text() {
            self.check_text(text)
        } else {
            GuardrailResult::Pass
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lortex_core::guardrail::Guardrail;
    use lortex_core::message::Message;

    #[test]
    fn name_returns_content_filter() {
        let filter = ContentFilter::new(vec![]);
        assert_eq!(filter.name(), "content_filter");
    }

    #[tokio::test]
    async fn pass_when_no_blocked_patterns() {
        let filter = ContentFilter::new(vec![]);
        let result = filter
            .check_input(&[Message::user("anything goes")])
            .await;
        assert!(result.is_pass());
    }

    #[tokio::test]
    async fn blocks_matching_pattern() {
        let filter = ContentFilter::new(vec!["forbidden".into()]);
        let result = filter
            .check_input(&[Message::user("this is forbidden content")])
            .await;
        assert!(result.is_block());
    }

    #[tokio::test]
    async fn case_insensitive_matching() {
        let filter = ContentFilter::new(vec!["blocked".into()]);
        let result = filter
            .check_input(&[Message::user("This is BLOCKED text")])
            .await;
        assert!(result.is_block());
    }

    #[tokio::test]
    async fn pass_when_no_match() {
        let filter = ContentFilter::new(vec!["forbidden".into()]);
        let result = filter
            .check_input(&[Message::user("perfectly fine content")])
            .await;
        assert!(result.is_pass());
    }

    #[tokio::test]
    async fn checks_all_messages_in_input() {
        let filter = ContentFilter::new(vec!["bad".into()]);
        let result = filter
            .check_input(&[
                Message::user("good message"),
                Message::user("this is bad"),
            ])
            .await;
        assert!(result.is_block());
    }

    #[tokio::test]
    async fn check_output_blocks_matching() {
        let filter = ContentFilter::new(vec!["secret".into()]);
        let result = filter
            .check_output(&Message::assistant("this contains a secret"))
            .await;
        assert!(result.is_block());
    }

    #[tokio::test]
    async fn check_output_passes_clean() {
        let filter = ContentFilter::new(vec!["secret".into()]);
        let result = filter
            .check_output(&Message::assistant("nothing wrong here"))
            .await;
        assert!(result.is_pass());
    }

    #[tokio::test]
    async fn check_output_passes_no_text() {
        let filter = ContentFilter::new(vec!["secret".into()]);
        let msg = Message::tool_result("call_1", serde_json::json!("ok"), false);
        let result = filter.check_output(&msg).await;
        assert!(result.is_pass());
    }

    #[test]
    fn add_pattern_builder() {
        let filter = ContentFilter::new(vec![])
            .add_pattern("bad")
            .add_pattern("evil");
        // Verify patterns were added by checking behavior
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(filter.check_input(&[Message::user("evil plan")]));
        assert!(result.is_block());
    }

    #[test]
    fn default_filter_has_no_patterns() {
        let filter = ContentFilter::default_filter();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt.block_on(filter.check_input(&[Message::user("anything")]));
        assert!(result.is_pass());
    }
}
