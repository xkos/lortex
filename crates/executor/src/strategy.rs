//! Execution strategies — pluggable patterns for how agents process tasks.

use std::sync::Arc;

use async_trait::async_trait;

use lortex_core::agent::{Agent, RunInput, RunOutput};
use lortex_core::error::LortexError;
use lortex_core::message::Message;

use crate::runner::Runner;

/// A strategy that defines how an agent processes a task.
#[async_trait]
pub trait ExecutionStrategy: Send + Sync {
    /// Execute the agent with the given input and context.
    async fn execute(
        &self,
        runner: &Runner,
        agent: &dyn Agent,
        input: RunInput,
    ) -> Result<RunOutput, LortexError>;
}

/// ReAct strategy: Reason -> Act -> Observe loop.
///
/// This is the default strategy used by the Runner. The agent loop continues
/// until the LLM produces a final answer (no tool calls) or the max iterations
/// limit is reached.
pub struct ReActStrategy {
    /// Maximum number of reason-act-observe iterations.
    pub max_iterations: usize,
}

impl ReActStrategy {
    pub fn new(max_iterations: usize) -> Self {
        Self { max_iterations }
    }
}

impl Default for ReActStrategy {
    fn default() -> Self {
        Self {
            max_iterations: 10,
        }
    }
}

#[async_trait]
impl ExecutionStrategy for ReActStrategy {
    async fn execute(
        &self,
        runner: &Runner,
        agent: &dyn Agent,
        input: RunInput,
    ) -> Result<RunOutput, LortexError> {
        // The Runner's default loop already implements ReAct.
        runner.run(agent, input).await
    }
}

/// Plan-and-Execute strategy: first plan, then execute each step.
///
/// Uses a planner agent to generate a structured plan, then executes each
/// step with the main agent (or specialized agents).
pub struct PlanAndExecuteStrategy {
    /// The planner agent that generates the execution plan.
    pub planner: Arc<dyn Agent>,

    /// Maximum number of steps in the plan.
    pub max_steps: usize,
}

impl PlanAndExecuteStrategy {
    pub fn new(planner: Arc<dyn Agent>, max_steps: usize) -> Self {
        Self { planner, max_steps }
    }
}

#[async_trait]
impl ExecutionStrategy for PlanAndExecuteStrategy {
    async fn execute(
        &self,
        runner: &Runner,
        agent: &dyn Agent,
        input: RunInput,
    ) -> Result<RunOutput, LortexError> {
        // Step 1: Generate a plan
        let plan_input = match &input {
            RunInput::Text(text) => RunInput::Text(format!(
                "Create a step-by-step plan to accomplish the following task. \
                 Return the plan as a numbered list.\n\nTask: {}",
                text
            )),
            RunInput::Messages(msgs) => {
                let mut plan_msgs = msgs.clone();
                if let Some(last) = plan_msgs.last_mut() {
                    if let Some(text) = last.text() {
                        let new_text = format!(
                            "Create a step-by-step plan to accomplish the following task. \
                             Return the plan as a numbered list.\n\nTask: {}",
                            text
                        );
                        *last = Message::user(new_text);
                    }
                }
                RunInput::Messages(plan_msgs)
            }
        };

        let plan_output = runner.run(&*self.planner, plan_input).await?;
        let plan_text = plan_output
            .message
            .text()
            .unwrap_or("No plan generated")
            .to_string();

        tracing::info!(plan = %plan_text, "Plan generated");

        // Step 2: Execute the plan with the main agent
        let execution_input = RunInput::Text(format!(
            "Execute the following plan step by step. Use the available tools as needed.\n\n\
             Original task: {}\n\nPlan:\n{}",
            match &input {
                RunInput::Text(t) => t.clone(),
                RunInput::Messages(msgs) => msgs
                    .last()
                    .and_then(|m| m.text())
                    .unwrap_or("")
                    .to_string(),
            },
            plan_text
        ));

        runner.run(agent, execution_input).await
    }
}
