//! Cost tracking — records token consumption and cost per LLM call.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::registry::CostProfile;

/// A single cost record from one LLM call.
#[derive(Debug, Clone)]
pub struct CostRecord {
    /// Provider name.
    pub provider: String,
    /// Model identifier.
    pub model: String,
    /// Input tokens consumed.
    pub input_tokens: u32,
    /// Output tokens consumed.
    pub output_tokens: u32,
    /// Computed cost in USD.
    pub cost_usd: f64,
}

/// Tracks cumulative cost across LLM calls.
pub struct CostTracker {
    /// All recorded cost entries.
    records: Arc<RwLock<Vec<CostRecord>>>,
    /// Optional budget limit in USD.
    budget_usd: Option<f64>,
    /// Running total cost in micro-USD (for atomic operations).
    total_micro_usd: AtomicU64,
}

impl CostTracker {
    /// Create a tracker with no budget limit.
    pub fn new() -> Self {
        Self {
            records: Arc::new(RwLock::new(Vec::new())),
            budget_usd: None,
            total_micro_usd: AtomicU64::new(0),
        }
    }

    /// Create a tracker with a budget limit.
    pub fn with_budget(budget_usd: f64) -> Self {
        Self {
            records: Arc::new(RwLock::new(Vec::new())),
            budget_usd: Some(budget_usd),
            total_micro_usd: AtomicU64::new(0),
        }
    }

    /// Compute cost for a single call given token counts and cost profile.
    pub fn compute_cost(
        input_tokens: u32,
        output_tokens: u32,
        cost_profile: &CostProfile,
    ) -> f64 {
        let input_cost = (input_tokens as f64 / 1_000_000.0) * cost_profile.input_per_million;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * cost_profile.output_per_million;
        input_cost + output_cost
    }

    /// Record a cost entry. Returns a `BudgetStatus` indicating budget health.
    pub async fn record(
        &self,
        provider: &str,
        model: &str,
        input_tokens: u32,
        output_tokens: u32,
        cost_profile: &CostProfile,
    ) -> BudgetStatus {
        let cost_usd = Self::compute_cost(input_tokens, output_tokens, cost_profile);

        let record = CostRecord {
            provider: provider.into(),
            model: model.into(),
            input_tokens,
            output_tokens,
            cost_usd,
        };

        self.records.write().await.push(record);

        // Update atomic total (store as micro-USD for precision)
        let micro = (cost_usd * 1_000_000.0) as u64;
        let new_total_micro = self.total_micro_usd.fetch_add(micro, Ordering::Relaxed) + micro;
        let new_total = new_total_micro as f64 / 1_000_000.0;

        match self.budget_usd {
            Some(budget) if new_total > budget => BudgetStatus::Exceeded {
                total: new_total,
                budget,
            },
            Some(budget) if new_total > budget * 0.8 => BudgetStatus::Warning {
                total: new_total,
                budget,
            },
            _ => BudgetStatus::Ok,
        }
    }

    /// Get the total cost so far in USD.
    pub fn total_cost(&self) -> f64 {
        self.total_micro_usd.load(Ordering::Relaxed) as f64 / 1_000_000.0
    }

    /// Get all recorded entries.
    pub async fn records(&self) -> Vec<CostRecord> {
        self.records.read().await.clone()
    }

    /// Get total cost for a specific model ("provider/model" key).
    pub async fn cost_by_model(&self, provider: &str, model: &str) -> f64 {
        self.records
            .read()
            .await
            .iter()
            .filter(|r| r.provider == provider && r.model == model)
            .map(|r| r.cost_usd)
            .sum()
    }

    /// Get total tokens consumed (input + output).
    pub async fn total_tokens(&self) -> (u64, u64) {
        let records = self.records.read().await;
        let input: u64 = records.iter().map(|r| r.input_tokens as u64).sum();
        let output: u64 = records.iter().map(|r| r.output_tokens as u64).sum();
        (input, output)
    }

    /// Reset all tracking data.
    pub async fn reset(&self) {
        self.records.write().await.clear();
        self.total_micro_usd.store(0, Ordering::Relaxed);
    }
}

impl Default for CostTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Budget health status after recording a cost.
#[derive(Debug, Clone, PartialEq)]
pub enum BudgetStatus {
    /// Under budget, no concerns.
    Ok,
    /// Over 80% of budget consumed.
    Warning { total: f64, budget: f64 },
    /// Budget exceeded.
    Exceeded { total: f64, budget: f64 },
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::CostProfile;

    fn gpt4o_cost() -> CostProfile {
        CostProfile {
            input_per_million: 2.5,
            output_per_million: 10.0,
        }
    }

    fn cheap_cost() -> CostProfile {
        CostProfile {
            input_per_million: 0.15,
            output_per_million: 0.6,
        }
    }

    #[test]
    fn compute_cost_basic() {
        let cost = CostTracker::compute_cost(1_000_000, 1_000_000, &gpt4o_cost());
        // 1M input * 2.5/M + 1M output * 10.0/M = 12.5
        assert!((cost - 12.5).abs() < 0.001);
    }

    #[test]
    fn compute_cost_zero_tokens() {
        let cost = CostTracker::compute_cost(0, 0, &gpt4o_cost());
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn compute_cost_small_call() {
        // 1000 input, 500 output with gpt4o pricing
        let cost = CostTracker::compute_cost(1000, 500, &gpt4o_cost());
        // 1000/1M * 2.5 + 500/1M * 10.0 = 0.0025 + 0.005 = 0.0075
        assert!((cost - 0.0075).abs() < 0.0001);
    }

    #[tokio::test]
    async fn record_and_query_total() {
        let tracker = CostTracker::new();
        tracker
            .record("openai", "gpt-4o", 1000, 500, &gpt4o_cost())
            .await;
        tracker
            .record("openai", "gpt-4o", 2000, 1000, &gpt4o_cost())
            .await;

        let total = tracker.total_cost();
        assert!(total > 0.0);

        let records = tracker.records().await;
        assert_eq!(records.len(), 2);
    }

    #[tokio::test]
    async fn cost_by_model() {
        let tracker = CostTracker::new();
        tracker
            .record("openai", "gpt-4o", 1000, 500, &gpt4o_cost())
            .await;
        tracker
            .record("openai", "gpt-4o-mini", 1000, 500, &cheap_cost())
            .await;

        let gpt4o = tracker.cost_by_model("openai", "gpt-4o").await;
        let mini = tracker.cost_by_model("openai", "gpt-4o-mini").await;
        assert!(gpt4o > mini);
    }

    #[tokio::test]
    async fn total_tokens() {
        let tracker = CostTracker::new();
        tracker
            .record("openai", "gpt-4o", 1000, 500, &gpt4o_cost())
            .await;
        tracker
            .record("openai", "gpt-4o", 2000, 1000, &gpt4o_cost())
            .await;

        let (input, output) = tracker.total_tokens().await;
        assert_eq!(input, 3000);
        assert_eq!(output, 1500);
    }

    #[tokio::test]
    async fn budget_ok() {
        let tracker = CostTracker::with_budget(1.0);
        let status = tracker
            .record("openai", "gpt-4o-mini", 1000, 500, &cheap_cost())
            .await;
        assert_eq!(status, BudgetStatus::Ok);
    }

    #[tokio::test]
    async fn budget_warning_at_80_percent() {
        // Budget: $0.01, cost per call with gpt4o: ~$0.0075
        let tracker = CostTracker::with_budget(0.01);
        // First call: $0.0075 = 75% → Ok
        let s1 = tracker
            .record("openai", "gpt-4o", 1000, 500, &gpt4o_cost())
            .await;
        assert_eq!(s1, BudgetStatus::Ok);

        // Second call: total ~$0.015 > $0.01 → Exceeded
        let s2 = tracker
            .record("openai", "gpt-4o", 1000, 500, &gpt4o_cost())
            .await;
        assert!(matches!(s2, BudgetStatus::Exceeded { .. }));
    }

    #[tokio::test]
    async fn budget_exceeded() {
        let tracker = CostTracker::with_budget(0.001);
        let status = tracker
            .record("openai", "gpt-4o", 1000, 500, &gpt4o_cost())
            .await;
        assert!(matches!(status, BudgetStatus::Exceeded { .. }));
    }

    #[tokio::test]
    async fn reset_clears_all() {
        let tracker = CostTracker::new();
        tracker
            .record("openai", "gpt-4o", 1000, 500, &gpt4o_cost())
            .await;
        assert!(tracker.total_cost() > 0.0);

        tracker.reset().await;
        assert_eq!(tracker.total_cost(), 0.0);
        assert!(tracker.records().await.is_empty());
    }

    #[tokio::test]
    async fn no_budget_never_warns() {
        let tracker = CostTracker::new();
        // Record a huge call
        let status = tracker
            .record("openai", "gpt-4o", 10_000_000, 5_000_000, &gpt4o_cost())
            .await;
        assert_eq!(status, BudgetStatus::Ok);
    }

    #[test]
    fn default_impl() {
        let tracker = CostTracker::default();
        assert_eq!(tracker.total_cost(), 0.0);
    }
}
