//! Model registry — model profiles, capabilities, and registration.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Capability scores for a model (0.0 - 1.0 per dimension).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Capabilities {
    /// Planning and decomposition ability.
    pub planning: f32,
    /// Logical reasoning ability.
    pub reasoning: f32,
    /// Code generation ability.
    pub coding: f32,
    /// Creative writing ability.
    pub creative: f32,
    /// Efficiency on simple/routine tasks.
    pub simple_task: f32,
}

/// Cost profile for a model (per million tokens).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CostProfile {
    /// Cost per million input tokens (USD).
    pub input_per_million: f64,
    /// Cost per million output tokens (USD).
    pub output_per_million: f64,
}

/// Supported modalities.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Modality {
    Text,
    Image,
    Audio,
    Video,
}

/// A registered model's profile describing its capabilities, cost, and constraints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelProfile {
    /// Provider name (e.g., "openai", "anthropic", "local").
    pub provider: String,
    /// Model identifier (e.g., "gpt-4o", "claude-sonnet-4-20250514").
    pub model: String,
    /// Capability scores.
    pub capabilities: Capabilities,
    /// Cost profile.
    pub cost: CostProfile,
    /// Approximate speed in tokens per second.
    pub speed: f32,
    /// Context window size in tokens.
    pub context_window: usize,
    /// Supported modalities.
    pub modalities: Vec<Modality>,
    /// Whether the model supports streaming.
    pub supports_streaming: bool,
    /// Whether the model supports tool/function calling.
    pub supports_tools: bool,
}

impl ModelProfile {
    /// Unique key for this model: "provider/model".
    pub fn key(&self) -> String {
        format!("{}/{}", self.provider, self.model)
    }
}

/// Registry of available models and their profiles.
#[derive(Debug, Default)]
pub struct ModelRegistry {
    models: HashMap<String, ModelProfile>,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a model profile.
    pub fn register(&mut self, profile: ModelProfile) {
        self.models.insert(profile.key(), profile);
    }

    /// Look up a model by "provider/model" key.
    pub fn get(&self, key: &str) -> Option<&ModelProfile> {
        self.models.get(key)
    }

    /// Look up a model by provider and model name separately.
    pub fn get_by_name(&self, provider: &str, model: &str) -> Option<&ModelProfile> {
        self.get(&format!("{provider}/{model}"))
    }

    /// List all registered models.
    pub fn all(&self) -> Vec<&ModelProfile> {
        self.models.values().collect()
    }

    /// Number of registered models.
    pub fn len(&self) -> usize {
        self.models.len()
    }

    pub fn is_empty(&self) -> bool {
        self.models.is_empty()
    }

    /// Remove a model by key.
    pub fn remove(&mut self, key: &str) -> Option<ModelProfile> {
        self.models.remove(key)
    }

    /// Find models that support a given modality.
    pub fn with_modality(&self, modality: &Modality) -> Vec<&ModelProfile> {
        self.models
            .values()
            .filter(|p| p.modalities.contains(modality))
            .collect()
    }

    /// Find models that support tool calling.
    pub fn with_tools(&self) -> Vec<&ModelProfile> {
        self.models
            .values()
            .filter(|p| p.supports_tools)
            .collect()
    }

    /// Find models whose context window is at least `min_tokens`.
    pub fn with_min_context(&self, min_tokens: usize) -> Vec<&ModelProfile> {
        self.models
            .values()
            .filter(|p| p.context_window >= min_tokens)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gpt4o() -> ModelProfile {
        ModelProfile {
            provider: "openai".into(),
            model: "gpt-4o".into(),
            capabilities: Capabilities {
                planning: 0.8,
                reasoning: 0.85,
                coding: 0.9,
                creative: 0.7,
                simple_task: 0.95,
            },
            cost: CostProfile {
                input_per_million: 2.5,
                output_per_million: 10.0,
            },
            speed: 80.0,
            context_window: 128_000,
            modalities: vec![Modality::Text, Modality::Image],
            supports_streaming: true,
            supports_tools: true,
        }
    }

    fn claude_sonnet() -> ModelProfile {
        ModelProfile {
            provider: "anthropic".into(),
            model: "claude-sonnet".into(),
            capabilities: Capabilities {
                planning: 0.85,
                reasoning: 0.9,
                coding: 0.95,
                creative: 0.8,
                simple_task: 0.9,
            },
            cost: CostProfile {
                input_per_million: 3.0,
                output_per_million: 15.0,
            },
            speed: 60.0,
            context_window: 200_000,
            modalities: vec![Modality::Text, Modality::Image],
            supports_streaming: true,
            supports_tools: true,
        }
    }

    fn cheap_model() -> ModelProfile {
        ModelProfile {
            provider: "openai".into(),
            model: "gpt-4o-mini".into(),
            capabilities: Capabilities {
                planning: 0.5,
                reasoning: 0.6,
                coding: 0.6,
                creative: 0.5,
                simple_task: 0.9,
            },
            cost: CostProfile {
                input_per_million: 0.15,
                output_per_million: 0.6,
            },
            speed: 150.0,
            context_window: 128_000,
            modalities: vec![Modality::Text],
            supports_streaming: true,
            supports_tools: true,
        }
    }

    #[test]
    fn model_profile_key() {
        let p = gpt4o();
        assert_eq!(p.key(), "openai/gpt-4o");
    }

    #[test]
    fn registry_new_is_empty() {
        let reg = ModelRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
    }

    #[test]
    fn register_and_get() {
        let mut reg = ModelRegistry::new();
        reg.register(gpt4o());
        assert_eq!(reg.len(), 1);

        let profile = reg.get("openai/gpt-4o").unwrap();
        assert_eq!(profile.model, "gpt-4o");
        assert_eq!(profile.provider, "openai");
    }

    #[test]
    fn get_by_name() {
        let mut reg = ModelRegistry::new();
        reg.register(claude_sonnet());

        let profile = reg.get_by_name("anthropic", "claude-sonnet").unwrap();
        assert_eq!(profile.model, "claude-sonnet");
    }

    #[test]
    fn get_nonexistent_returns_none() {
        let reg = ModelRegistry::new();
        assert!(reg.get("openai/gpt-5").is_none());
    }

    #[test]
    fn register_overwrites_same_key() {
        let mut reg = ModelRegistry::new();
        reg.register(gpt4o());
        let mut updated = gpt4o();
        updated.speed = 100.0;
        reg.register(updated);
        assert_eq!(reg.len(), 1);
        assert_eq!(reg.get("openai/gpt-4o").unwrap().speed, 100.0);
    }

    #[test]
    fn all_returns_all_models() {
        let mut reg = ModelRegistry::new();
        reg.register(gpt4o());
        reg.register(claude_sonnet());
        reg.register(cheap_model());
        assert_eq!(reg.all().len(), 3);
    }

    #[test]
    fn remove_model() {
        let mut reg = ModelRegistry::new();
        reg.register(gpt4o());
        let removed = reg.remove("openai/gpt-4o");
        assert!(removed.is_some());
        assert!(reg.is_empty());
    }

    #[test]
    fn remove_nonexistent_returns_none() {
        let mut reg = ModelRegistry::new();
        assert!(reg.remove("ghost").is_none());
    }

    #[test]
    fn with_modality_filters() {
        let mut reg = ModelRegistry::new();
        reg.register(gpt4o());       // Text + Image
        reg.register(cheap_model());  // Text only

        let image_models = reg.with_modality(&Modality::Image);
        assert_eq!(image_models.len(), 1);
        assert_eq!(image_models[0].model, "gpt-4o");

        let text_models = reg.with_modality(&Modality::Text);
        assert_eq!(text_models.len(), 2);
    }

    #[test]
    fn with_tools_filters() {
        let mut reg = ModelRegistry::new();
        reg.register(gpt4o());
        let mut no_tools = cheap_model();
        no_tools.supports_tools = false;
        no_tools.model = "no-tools".into();
        reg.register(no_tools);

        let tool_models = reg.with_tools();
        assert_eq!(tool_models.len(), 1);
    }

    #[test]
    fn with_min_context_filters() {
        let mut reg = ModelRegistry::new();
        reg.register(gpt4o());          // 128k
        reg.register(claude_sonnet());  // 200k

        let large = reg.with_min_context(150_000);
        assert_eq!(large.len(), 1);
        assert_eq!(large[0].model, "claude-sonnet");

        let all = reg.with_min_context(100_000);
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn capabilities_default() {
        let caps = Capabilities::default();
        assert_eq!(caps.planning, 0.0);
        assert_eq!(caps.reasoning, 0.0);
    }

    #[test]
    fn cost_profile_default() {
        let cost = CostProfile::default();
        assert_eq!(cost.input_per_million, 0.0);
        assert_eq!(cost.output_per_million, 0.0);
    }
}
