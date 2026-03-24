//! Orchestrator — coordinates multi-agent execution.

use std::sync::Arc;

use tracing::info;

use lortex_core::agent::{Agent, RunInput, RunOutput};
use lortex_core::error::LortexError;

use lortex_executor::Runner;

use crate::patterns::OrchestrationPattern;

/// Orchestrator manages multi-agent collaboration using a specified pattern.
pub struct Orchestrator {
    /// The orchestration pattern to use.
    pattern: OrchestrationPattern,

    /// The runner for executing individual agents.
    runner: Arc<Runner>,
}

impl Orchestrator {
    /// Create a new orchestrator.
    pub fn new(pattern: OrchestrationPattern, runner: Arc<Runner>) -> Self {
        Self { pattern, runner }
    }

    /// Create a builder.
    pub fn builder() -> OrchestratorBuilder {
        OrchestratorBuilder::new()
    }

    /// Run the orchestrator with the given input.
    pub async fn run(&self, input: impl Into<RunInput>) -> Result<RunOutput, LortexError> {
        let input = input.into();
        match &self.pattern {
            OrchestrationPattern::Router { triage_agent } => {
                self.run_router(&**triage_agent, input).await
            }
            OrchestrationPattern::Pipeline { stages } => self.run_pipeline(stages, input).await,
            OrchestrationPattern::Parallel {
                agents,
                aggregator,
            } => self.run_parallel(agents, &**aggregator, input).await,
            OrchestrationPattern::Hierarchical {
                supervisor,
                workers,
            } => self.run_hierarchical(&**supervisor, workers, input).await,
        }
    }

    /// Router pattern: the triage agent decides which specialist handles the task.
    /// The triage agent uses handoffs to delegate.
    async fn run_router(
        &self,
        triage_agent: &dyn Agent,
        input: RunInput,
    ) -> Result<RunOutput, LortexError> {
        info!(agent = triage_agent.name(), "Router: dispatching via triage agent");
        self.runner.run(triage_agent, input).await
    }

    /// Pipeline pattern: process through each stage sequentially.
    async fn run_pipeline(
        &self,
        stages: &[Arc<dyn Agent>],
        input: RunInput,
    ) -> Result<RunOutput, LortexError> {
        let mut current_input = input;

        for (i, stage) in stages.iter().enumerate() {
            info!(
                stage = i,
                agent = stage.name(),
                total = stages.len(),
                "Pipeline: executing stage"
            );

            let output = self.runner.run(&**stage, current_input).await?;

            // Pass the output as input to the next stage
            if i < stages.len() - 1 {
                let text = output
                    .message
                    .text()
                    .unwrap_or("")
                    .to_string();
                current_input = RunInput::Text(text);
            } else {
                return Ok(output);
            }
        }

        Err(LortexError::Other("Pipeline has no stages".into()))
    }

    /// Parallel pattern: run all agents concurrently and aggregate results.
    async fn run_parallel(
        &self,
        agents: &[Arc<dyn Agent>],
        aggregator: &dyn Agent,
        input: RunInput,
    ) -> Result<RunOutput, LortexError> {
        info!(
            agents = agents.len(),
            "Parallel: running agents concurrently"
        );

        // Spawn all agents concurrently
        let mut handles = vec![];
        for agent in agents {
            let runner = self.runner.clone();
            let agent = agent.clone();
            let input = input.clone();
            handles.push(tokio::spawn(async move {
                let result = runner.run(&*agent, input).await;
                (agent.name().to_string(), result)
            }));
        }

        // Collect results
        let mut results = vec![];
        for handle in handles {
            match handle.await {
                Ok((name, Ok(output))) => {
                    let text = output.message.text().unwrap_or("").to_string();
                    results.push(format!("[{}]: {}", name, text));
                }
                Ok((name, Err(e))) => {
                    results.push(format!("[{}]: Error: {}", name, e));
                }
                Err(e) => {
                    results.push(format!("[unknown]: Join error: {}", e));
                }
            }
        }

        // Aggregate results
        let aggregation_input = format!(
            "The following are results from multiple agents. Please synthesize them \
             into a single coherent response.\n\n{}",
            results.join("\n\n")
        );

        info!("Parallel: aggregating results");
        self.runner.run(aggregator, aggregation_input).await
    }

    /// Hierarchical pattern: supervisor coordinates workers.
    async fn run_hierarchical(
        &self,
        supervisor: &dyn Agent,
        workers: &[Arc<dyn Agent>],
        input: RunInput,
    ) -> Result<RunOutput, LortexError> {
        info!(
            supervisor = supervisor.name(),
            workers = workers.len(),
            "Hierarchical: supervisor coordinating workers"
        );

        // The supervisor agent has handoffs to worker agents.
        // The Runner's built-in handoff mechanism handles the delegation.
        self.runner.run(supervisor, input).await
    }
}

/// Builder for constructing an Orchestrator.
pub struct OrchestratorBuilder {
    pattern: Option<OrchestrationPattern>,
    runner: Option<Arc<Runner>>,
}

impl OrchestratorBuilder {
    pub fn new() -> Self {
        Self {
            pattern: None,
            runner: None,
        }
    }

    pub fn pattern(mut self, pattern: OrchestrationPattern) -> Self {
        self.pattern = Some(pattern);
        self
    }

    pub fn runner(mut self, runner: Arc<Runner>) -> Self {
        self.runner = Some(runner);
        self
    }

    pub fn build(self) -> Result<Orchestrator, String> {
        Ok(Orchestrator {
            pattern: self.pattern.ok_or("Orchestration pattern is required")?,
            runner: self.runner.ok_or("Runner is required")?,
        })
    }
}

impl Default for OrchestratorBuilder {
    fn default() -> Self {
        Self::new()
    }
}
