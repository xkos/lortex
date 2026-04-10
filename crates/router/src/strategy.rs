//! Routing strategies — pluggable model selection logic.

use lortex_core::provider::CompletionRequest;

use crate::registry::ModelRegistry;

/// The result of a routing decision.
#[derive(Debug, Clone)]
pub struct ModelSelection {
    /// The selected provider name.
    pub provider: String,
    /// The selected model identifier.
    pub model: String,
    /// Registry key ("provider/model").
    pub key: String,
}

/// A routing request wrapping the original completion request with routing context.
#[derive(Debug)]
pub struct RoutingRequest<'a> {
    /// The original completion request.
    pub request: &'a CompletionRequest,
    /// Number of tools in the request (may influence model selection).
    pub tool_count: usize,
    /// Estimated input tokens (rough heuristic).
    pub estimated_input_tokens: usize,
}

impl<'a> RoutingRequest<'a> {
    pub fn from_completion(request: &'a CompletionRequest) -> Self {
        let tool_count = request.tools.len();
        // Rough estimate: serialize messages to string and count chars / 4
        let estimated_input_tokens = request
            .messages
            .iter()
            .filter_map(|m| m.text())
            .map(|t| t.len() / 4)
            .sum();
        Self {
            request,
            tool_count,
            estimated_input_tokens,
        }
    }
}

/// Trait for model routing strategies.
pub trait RoutingStrategy: Send + Sync {
    /// Select a model given the routing request and available models.
    fn select(
        &self,
        request: &RoutingRequest,
        registry: &ModelRegistry,
    ) -> Result<ModelSelection, RoutingError>;
}

/// Errors from routing.
#[derive(Debug, Clone, thiserror::Error)]
pub enum RoutingError {
    #[error("No model available: {0}")]
    NoModelAvailable(String),

    #[error("Model not found in registry: {0}")]
    ModelNotFound(String),
}

/// Fixed routing strategy — always routes to a specific provider/model.
pub struct FixedRouter {
    provider: String,
    model: String,
}

impl FixedRouter {
    pub fn new(provider: impl Into<String>, model: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            model: model.into(),
        }
    }
}

impl RoutingStrategy for FixedRouter {
    fn select(
        &self,
        _request: &RoutingRequest,
        registry: &ModelRegistry,
    ) -> Result<ModelSelection, RoutingError> {
        let key = format!("{}/{}", self.provider, self.model);
        // Verify the model exists in the registry
        registry
            .get(&key)
            .ok_or_else(|| RoutingError::ModelNotFound(key.clone()))?;

        Ok(ModelSelection {
            provider: self.provider.clone(),
            model: self.model.clone(),
            key,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{Capabilities, CostProfile, Modality, ModelProfile, ModelRegistry};
    use lortex_core::message::Message;
    use lortex_core::provider::CompletionRequest;
    use serde_json::Value;

    fn test_registry() -> ModelRegistry {
        let mut reg = ModelRegistry::new();
        reg.register(ModelProfile {
            provider: "openai".into(),
            model: "gpt-4o".into(),
            capabilities: Capabilities::default(),
            cost: CostProfile::default(),
            speed: 80.0,
            context_window: 128_000,
            modalities: vec![Modality::Text],
            supports_streaming: true,
            supports_tools: true,
        });
        reg.register(ModelProfile {
            provider: "anthropic".into(),
            model: "claude-sonnet".into(),
            capabilities: Capabilities::default(),
            cost: CostProfile::default(),
            speed: 60.0,
            context_window: 200_000,
            modalities: vec![Modality::Text],
            supports_streaming: true,
            supports_tools: true,
        });
        reg
    }

    fn test_request() -> CompletionRequest {
        CompletionRequest {
            model: "any".into(),
            messages: vec![Message::user("hello")],
            tools: vec![],
            temperature: 0.7,
            max_tokens: None,
            stop: vec![],
            extra: Value::Null,
        }
    }

    #[test]
    fn routing_request_from_completion() {
        let req = test_request();
        let routing = RoutingRequest::from_completion(&req);
        assert_eq!(routing.tool_count, 0);
        assert!(routing.estimated_input_tokens > 0);
    }

    #[test]
    fn fixed_router_selects_specified_model() {
        let reg = test_registry();
        let router = FixedRouter::new("openai", "gpt-4o");
        let req = test_request();
        let routing = RoutingRequest::from_completion(&req);

        let selection = router.select(&routing, &reg).unwrap();
        assert_eq!(selection.provider, "openai");
        assert_eq!(selection.model, "gpt-4o");
        assert_eq!(selection.key, "openai/gpt-4o");
    }

    #[test]
    fn fixed_router_errors_on_missing_model() {
        let reg = test_registry();
        let router = FixedRouter::new("openai", "gpt-5");
        let req = test_request();
        let routing = RoutingRequest::from_completion(&req);

        let result = router.select(&routing, &reg);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Model not found"));
    }

    #[test]
    fn fixed_router_ignores_request_content() {
        let reg = test_registry();
        let router = FixedRouter::new("anthropic", "claude-sonnet");

        // Different request content should not affect selection
        let mut req = test_request();
        req.messages = vec![
            Message::user("complex task requiring deep reasoning"),
            Message::user("with multiple messages"),
        ];
        let routing = RoutingRequest::from_completion(&req);

        let selection = router.select(&routing, &reg).unwrap();
        assert_eq!(selection.provider, "anthropic");
        assert_eq!(selection.model, "claude-sonnet");
    }

    #[test]
    fn model_selection_debug() {
        let sel = ModelSelection {
            provider: "openai".into(),
            model: "gpt-4o".into(),
            key: "openai/gpt-4o".into(),
        };
        let dbg = format!("{:?}", sel);
        assert!(dbg.contains("gpt-4o"));
    }
}
