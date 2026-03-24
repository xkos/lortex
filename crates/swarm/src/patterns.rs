//! Orchestration patterns for multi-agent coordination.

use std::sync::Arc;
use lortex_core::agent::Agent;

/// Available orchestration patterns for multi-agent systems.
pub enum OrchestrationPattern {
    /// Router pattern: a triage agent dispatches tasks to specialized agents.
    ///
    /// The triage agent inspects the input and decides which specialist
    /// should handle it, using handoffs.
    Router {
        triage_agent: Arc<dyn Agent>,
    },

    /// Pipeline pattern: agents process tasks sequentially.
    ///
    /// Each agent's output becomes the next agent's input.
    /// Useful for workflows like: retrieve -> analyze -> draft -> review.
    Pipeline {
        stages: Vec<Arc<dyn Agent>>,
    },

    /// Parallel pattern: multiple agents process the same input concurrently.
    ///
    /// Results are aggregated by a dedicated aggregator agent.
    Parallel {
        agents: Vec<Arc<dyn Agent>>,
        aggregator: Arc<dyn Agent>,
    },

    /// Hierarchical pattern: a supervisor coordinates worker agents.
    ///
    /// The supervisor decides which workers to invoke and how to combine results.
    Hierarchical {
        supervisor: Arc<dyn Agent>,
        workers: Vec<Arc<dyn Agent>>,
    },
}
