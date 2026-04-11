//! Router — a smart Provider that routes requests to different models.

use std::collections::HashMap;
use std::fmt;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use futures::Stream;

use lortex_core::error::ProviderError;
use lortex_core::provider::{
    CompletionRequest, CompletionResponse, Provider, ProviderCapabilities, StreamEvent,
};

use crate::cost::CostTracker;
use crate::registry::ModelRegistry;
use crate::strategy::{RoutingRequest, RoutingStrategy};

/// A Router that implements `Provider` by delegating to registered providers
/// based on a routing strategy.
pub struct Router {
    /// Named providers keyed by provider name (e.g., "openai", "anthropic").
    providers: HashMap<String, Arc<dyn Provider>>,
    /// Model registry with capability/cost metadata.
    registry: ModelRegistry,
    /// The routing strategy to use.
    strategy: Box<dyn RoutingStrategy>,
    /// Cost tracker.
    cost_tracker: Arc<CostTracker>,
}

impl fmt::Debug for Router {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Router")
            .field("providers", &self.providers.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Router {
    pub fn builder() -> RouterBuilder {
        RouterBuilder::new()
    }

    /// Get a reference to the cost tracker.
    pub fn cost_tracker(&self) -> &CostTracker {
        &self.cost_tracker
    }

    /// Get a reference to the model registry.
    pub fn registry(&self) -> &ModelRegistry {
        &self.registry
    }
}

#[async_trait]
impl Provider for Router {
    fn name(&self) -> &str {
        "router"
    }

    async fn complete(
        &self,
        request: CompletionRequest,
    ) -> Result<CompletionResponse, ProviderError> {
        // 1. Route: select a model
        let routing_req = RoutingRequest::from_completion(&request);
        let selection = self
            .strategy
            .select(&routing_req, &self.registry)
            .map_err(|e| ProviderError::ModelNotSupported(e.to_string()))?;

        // 2. Find the provider
        let provider = self
            .providers
            .get(&selection.provider)
            .ok_or_else(|| {
                ProviderError::ModelNotSupported(format!(
                    "Provider '{}' not registered",
                    selection.provider
                ))
            })?;

        // 3. Override the model in the request
        let mut routed_request = request;
        routed_request.model = selection.model.clone();

        // 4. Call the provider
        let response = provider.complete(routed_request).await?;

        // 5. Record cost
        if let Some(usage) = &response.usage {
            if let Some(profile) = self.registry.get(&selection.key) {
                self.cost_tracker
                    .record(
                        &selection.provider,
                        &selection.model,
                        usage.prompt_tokens,
                        usage.completion_tokens,
                        &profile.cost,
                    )
                    .await;
            }
        }

        Ok(response)
    }

    fn complete_stream(
        &self,
        _request: CompletionRequest,
    ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, ProviderError>> + Send + '_>> {
        // Streaming through router: delegate to selected provider
        // For now, return empty stream (same as individual providers' simplified impl)
        Box::pin(futures::stream::empty())
    }

    fn capabilities(&self) -> ProviderCapabilities {
        // Aggregate capabilities: if any provider supports a feature, the router does too
        let mut caps = ProviderCapabilities::default();
        for provider in self.providers.values() {
            let pc = provider.capabilities();
            caps.streaming = caps.streaming || pc.streaming;
            caps.tool_calling = caps.tool_calling || pc.tool_calling;
            caps.vision = caps.vision || pc.vision;
            caps.embeddings = caps.embeddings || pc.embeddings;
            caps.structured_output = caps.structured_output || pc.structured_output;
        }
        caps
    }
}

/// Builder for constructing a Router.
pub struct RouterBuilder {
    providers: HashMap<String, Arc<dyn Provider>>,
    registry: ModelRegistry,
    strategy: Option<Box<dyn RoutingStrategy>>,
    cost_tracker: Option<Arc<CostTracker>>,
}

impl RouterBuilder {
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
            registry: ModelRegistry::new(),
            strategy: None,
            cost_tracker: None,
        }
    }

    /// Register a provider by name.
    pub fn provider(mut self, name: impl Into<String>, provider: Arc<dyn Provider>) -> Self {
        self.providers.insert(name.into(), provider);
        self
    }

    /// Set the model registry.
    pub fn registry(mut self, registry: ModelRegistry) -> Self {
        self.registry = registry;
        self
    }

    /// Set the routing strategy.
    pub fn strategy(mut self, strategy: impl RoutingStrategy + 'static) -> Self {
        self.strategy = Some(Box::new(strategy));
        self
    }

    /// Set a custom cost tracker.
    pub fn cost_tracker(mut self, tracker: Arc<CostTracker>) -> Self {
        self.cost_tracker = Some(tracker);
        self
    }

    pub fn build(self) -> Result<Router, String> {
        let strategy = self.strategy.ok_or("Routing strategy is required")?;
        if self.providers.is_empty() {
            return Err("At least one provider is required".into());
        }

        Ok(Router {
            providers: self.providers,
            registry: self.registry,
            strategy,
            cost_tracker: self.cost_tracker.unwrap_or_else(|| Arc::new(CostTracker::new())),
        })
    }
}

impl Default for RouterBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{Capabilities, CostProfile, Modality, ModelProfile};
    use crate::strategy::FixedRouter;
    use lortex_core::message::Message;
    use lortex_core::provider::{CompletionResponse, Usage};
    use serde_json::Value;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // --- Mock Provider ---

    struct MockProvider {
        provider_name: String,
        responses: Vec<CompletionResponse>,
        call_count: AtomicUsize,
    }

    impl MockProvider {
        fn new(name: &str, responses: Vec<CompletionResponse>) -> Self {
            Self {
                provider_name: name.into(),
                responses,
                call_count: AtomicUsize::new(0),
            }
        }

        fn text(name: &str, text: &str) -> Self {
            Self::new(
                name,
                vec![CompletionResponse {
                    message: Message::assistant(text),
                    usage: Some(Usage {
                        prompt_tokens: 100,
                        completion_tokens: 50,
                        total_tokens: 150,
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 0,
                    }),
                    finish_reason: None,
                    model: "mock-model".into(),
                }],
            )
        }

        fn calls(&self) -> usize {
            self.call_count.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl Provider for MockProvider {
        fn name(&self) -> &str {
            &self.provider_name
        }

        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> Result<CompletionResponse, ProviderError> {
            let idx = self.call_count.fetch_add(1, Ordering::SeqCst);
            self.responses
                .get(idx)
                .cloned()
                .ok_or_else(|| ProviderError::InvalidResponse("No more responses".into()))
        }

        fn complete_stream(
            &self,
            _request: CompletionRequest,
        ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, ProviderError>> + Send + '_>> {
            Box::pin(futures::stream::empty())
        }

        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities {
                tool_calling: true,
                ..Default::default()
            }
        }
    }

    fn test_registry() -> ModelRegistry {
        let mut reg = ModelRegistry::new();
        reg.register(ModelProfile {
            provider: "openai".into(),
            model: "gpt-4o".into(),
            capabilities: Capabilities::default(),
            cost: CostProfile {
                input_per_million: 2.5,
                output_per_million: 10.0,
            },
            speed: 80.0,
            context_window: 128_000,
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
    fn builder_requires_strategy() {
        let provider = Arc::new(MockProvider::text("openai", "hi"));
        let result = RouterBuilder::new()
            .provider("openai", provider)
            .build();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("strategy"));
    }

    #[test]
    fn builder_requires_provider() {
        let result = RouterBuilder::new()
            .strategy(FixedRouter::new("openai", "gpt-4o"))
            .build();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("provider"));
    }

    #[test]
    fn builder_success() {
        let provider = Arc::new(MockProvider::text("openai", "hi"));
        let router = RouterBuilder::new()
            .provider("openai", provider)
            .registry(test_registry())
            .strategy(FixedRouter::new("openai", "gpt-4o"))
            .build();
        assert!(router.is_ok());
    }

    #[tokio::test]
    async fn routes_to_correct_provider() {
        let openai = Arc::new(MockProvider::text("openai", "from openai"));
        let anthropic = Arc::new(MockProvider::text("anthropic", "from anthropic"));

        let router = RouterBuilder::new()
            .provider("openai", openai.clone())
            .provider("anthropic", anthropic.clone())
            .registry(test_registry())
            .strategy(FixedRouter::new("openai", "gpt-4o"))
            .build()
            .unwrap();

        let response = router.complete(test_request()).await.unwrap();
        assert_eq!(response.message.text(), Some("from openai"));
        assert_eq!(openai.calls(), 1);
        assert_eq!(anthropic.calls(), 0);
    }

    #[tokio::test]
    async fn records_cost_after_call() {
        let provider = Arc::new(MockProvider::text("openai", "response"));
        let tracker = Arc::new(CostTracker::new());

        let router = RouterBuilder::new()
            .provider("openai", provider)
            .registry(test_registry())
            .strategy(FixedRouter::new("openai", "gpt-4o"))
            .cost_tracker(tracker.clone())
            .build()
            .unwrap();

        router.complete(test_request()).await.unwrap();

        assert!(tracker.total_cost() > 0.0);
        let records = tracker.records().await;
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].provider, "openai");
        assert_eq!(records[0].model, "gpt-4o");
    }

    #[tokio::test]
    async fn errors_on_missing_provider() {
        let provider = Arc::new(MockProvider::text("openai", "hi"));

        // Registry has openai/gpt-4o but strategy routes to anthropic/claude
        let mut reg = ModelRegistry::new();
        reg.register(ModelProfile {
            provider: "anthropic".into(),
            model: "claude".into(),
            capabilities: Capabilities::default(),
            cost: CostProfile::default(),
            speed: 60.0,
            context_window: 200_000,
            modalities: vec![Modality::Text],
            supports_streaming: true,
            supports_tools: true,
        });

        let router = RouterBuilder::new()
            .provider("openai", provider)
            .registry(reg)
            .strategy(FixedRouter::new("anthropic", "claude"))
            .build()
            .unwrap();

        let result = router.complete(test_request()).await;
        assert!(result.is_err());
    }

    #[test]
    fn router_name() {
        let provider = Arc::new(MockProvider::text("openai", "hi"));
        let router = RouterBuilder::new()
            .provider("openai", provider)
            .registry(test_registry())
            .strategy(FixedRouter::new("openai", "gpt-4o"))
            .build()
            .unwrap();
        assert_eq!(router.name(), "router");
    }

    #[test]
    fn capabilities_aggregated() {
        let p1 = Arc::new(MockProvider::text("a", "hi"));
        let router = RouterBuilder::new()
            .provider("a", p1)
            .registry(test_registry())
            .strategy(FixedRouter::new("openai", "gpt-4o"))
            .build()
            .unwrap();

        let caps = router.capabilities();
        assert!(caps.tool_calling); // MockProvider has tool_calling = true
    }

    #[tokio::test]
    async fn cost_tracker_accessible() {
        let provider = Arc::new(MockProvider::text("openai", "hi"));
        let router = RouterBuilder::new()
            .provider("openai", provider)
            .registry(test_registry())
            .strategy(FixedRouter::new("openai", "gpt-4o"))
            .build()
            .unwrap();

        assert_eq!(router.cost_tracker().total_cost(), 0.0);
        router.complete(test_request()).await.unwrap();
        assert!(router.cost_tracker().total_cost() > 0.0);
    }
}
