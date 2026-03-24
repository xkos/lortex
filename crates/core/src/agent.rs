//! Agent trait and related types.
//!
//! An Agent is a declarative configuration that combines:
//! - A system prompt (instructions)
//! - A model identifier
//! - A set of tools
//! - Optional handoffs to other agents
//! - Optional guardrails
//!
//! Agents are NOT executors — they are configurations consumed by a Runner.

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use crate::guardrail::Guardrail;
use crate::message::Message;
use crate::tool::Tool;

/// The core Agent trait. Agents declare their configuration;
/// actual execution is handled by the Runner.
pub trait Agent: Send + Sync {
    /// Unique name for this agent.
    fn name(&self) -> &str;

    /// System instructions (system prompt).
    fn instructions(&self) -> &str;

    /// Model identifier (e.g., "gpt-4o", "claude-sonnet-4-20250514").
    fn model(&self) -> &str;

    /// Available tools for this agent.
    fn tools(&self) -> Vec<Arc<dyn Tool>>;

    /// Agents that this agent can hand off to.
    fn handoffs(&self) -> Vec<Handoff> {
        vec![]
    }

    /// Input guardrails.
    fn input_guardrails(&self) -> Vec<Arc<dyn Guardrail>> {
        vec![]
    }

    /// Output guardrails.
    fn output_guardrails(&self) -> Vec<Arc<dyn Guardrail>> {
        vec![]
    }

    /// Optional hooks for lifecycle events.
    fn hooks(&self) -> Option<Arc<dyn AgentHooks>> {
        None
    }
}

impl fmt::Debug for dyn Agent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Agent")
            .field("name", &self.name())
            .field("model", &self.model())
            .finish()
    }
}

/// A handoff descriptor: tells the Runner which agent to delegate to.
/// Handoffs are presented to the LLM as special tools.
#[derive(Clone)]
pub struct Handoff {
    /// The target agent to hand off to.
    pub target: Arc<dyn Agent>,

    /// The tool name the LLM sees (e.g., "transfer_to_code_agent").
    pub tool_name: String,

    /// Description of when to use this handoff.
    pub tool_description: String,

    /// Optional filter that selects which messages pass to the target agent.
    pub input_filter: Option<Arc<dyn Fn(&[Message]) -> Vec<Message> + Send + Sync>>,
}

impl fmt::Debug for Handoff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Handoff")
            .field("target", &self.target.name())
            .field("tool_name", &self.tool_name)
            .finish()
    }
}

/// Lifecycle hooks for agent events.
#[async_trait]
pub trait AgentHooks: Send + Sync {
    /// Called before the agent starts processing.
    async fn on_start(&self, _agent_name: &str, _input: &[Message]) {}

    /// Called after each LLM response.
    async fn on_llm_response(&self, _agent_name: &str, _response: &Message) {}

    /// Called before a tool is executed.
    async fn on_tool_start(&self, _agent_name: &str, _tool_name: &str, _args: &Value) {}

    /// Called after a tool finishes execution.
    async fn on_tool_end(
        &self,
        _agent_name: &str,
        _tool_name: &str,
        _result: &crate::tool::ToolOutput,
    ) {
    }

    /// Called when a handoff occurs.
    async fn on_handoff(&self, _from: &str, _to: &str) {}

    /// Called when the agent finishes processing.
    async fn on_end(&self, _agent_name: &str, _output: &Message) {}
}

/// Input to an agent run.
#[derive(Debug, Clone)]
pub enum RunInput {
    /// A simple text input.
    Text(String),

    /// One or more messages.
    Messages(Vec<Message>),
}

impl From<String> for RunInput {
    fn from(s: String) -> Self {
        RunInput::Text(s)
    }
}

impl From<&str> for RunInput {
    fn from(s: &str) -> Self {
        RunInput::Text(s.to_string())
    }
}

impl From<Vec<Message>> for RunInput {
    fn from(msgs: Vec<Message>) -> Self {
        RunInput::Messages(msgs)
    }
}

/// Output from an agent run.
#[derive(Debug, Clone)]
pub struct RunOutput {
    /// The final response message.
    pub message: Message,

    /// The agent that produced this output.
    pub agent_name: String,

    /// All messages from the conversation (including intermediate tool calls).
    pub messages: Vec<Message>,

    /// Metadata about the run.
    pub metadata: HashMap<String, Value>,
}

/// Context available during an agent run.
#[derive(Debug, Clone)]
pub struct RunContext {
    /// Current session ID.
    pub session_id: String,

    /// The conversation messages accumulated so far.
    pub messages: Vec<Message>,

    /// Arbitrary context data.
    pub data: HashMap<String, Value>,
}

impl RunContext {
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            messages: vec![],
            data: HashMap::new(),
        }
    }
}

/// A default agent implementation using a builder pattern.
pub struct SimpleAgent {
    name: String,
    instructions: String,
    model: String,
    tools: Vec<Arc<dyn Tool>>,
    handoffs: Vec<Handoff>,
    input_guardrails: Vec<Arc<dyn Guardrail>>,
    output_guardrails: Vec<Arc<dyn Guardrail>>,
    hooks: Option<Arc<dyn AgentHooks>>,
}

impl Agent for SimpleAgent {
    fn name(&self) -> &str {
        &self.name
    }

    fn instructions(&self) -> &str {
        &self.instructions
    }

    fn model(&self) -> &str {
        &self.model
    }

    fn tools(&self) -> Vec<Arc<dyn Tool>> {
        self.tools.clone()
    }

    fn handoffs(&self) -> Vec<Handoff> {
        self.handoffs.clone()
    }

    fn input_guardrails(&self) -> Vec<Arc<dyn Guardrail>> {
        self.input_guardrails.clone()
    }

    fn output_guardrails(&self) -> Vec<Arc<dyn Guardrail>> {
        self.output_guardrails.clone()
    }

    fn hooks(&self) -> Option<Arc<dyn AgentHooks>> {
        self.hooks.clone()
    }
}

/// Builder for constructing a `SimpleAgent`.
pub struct AgentBuilder {
    name: Option<String>,
    instructions: Option<String>,
    model: Option<String>,
    tools: Vec<Arc<dyn Tool>>,
    handoffs: Vec<Handoff>,
    input_guardrails: Vec<Arc<dyn Guardrail>>,
    output_guardrails: Vec<Arc<dyn Guardrail>>,
    hooks: Option<Arc<dyn AgentHooks>>,
}

impl AgentBuilder {
    pub fn new() -> Self {
        Self {
            name: None,
            instructions: None,
            model: None,
            tools: vec![],
            handoffs: vec![],
            input_guardrails: vec![],
            output_guardrails: vec![],
            hooks: None,
        }
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = Some(instructions.into());
        self
    }

    pub fn model(mut self, model: impl Into<String>) -> Self {
        self.model = Some(model.into());
        self
    }

    pub fn tool(mut self, tool: Arc<dyn Tool>) -> Self {
        self.tools.push(tool);
        self
    }

    pub fn tools(mut self, tools: Vec<Arc<dyn Tool>>) -> Self {
        self.tools.extend(tools);
        self
    }

    pub fn handoff(mut self, handoff: Handoff) -> Self {
        self.handoffs.push(handoff);
        self
    }

    /// Convenience method: create a handoff from an agent.
    pub fn handoff_to(self, target: Arc<dyn Agent>) -> Self {
        let tool_name = format!("transfer_to_{}", target.name());
        let tool_description = format!(
            "Hand off the conversation to the {} agent.",
            target.name()
        );
        self.handoff(Handoff {
            target,
            tool_name,
            tool_description,
            input_filter: None,
        })
    }

    pub fn input_guardrail(mut self, guardrail: Arc<dyn Guardrail>) -> Self {
        self.input_guardrails.push(guardrail);
        self
    }

    pub fn output_guardrail(mut self, guardrail: Arc<dyn Guardrail>) -> Self {
        self.output_guardrails.push(guardrail);
        self
    }

    pub fn hooks(mut self, hooks: Arc<dyn AgentHooks>) -> Self {
        self.hooks = Some(hooks);
        self
    }

    pub fn build(self) -> Result<SimpleAgent, String> {
        Ok(SimpleAgent {
            name: self.name.ok_or("Agent name is required")?,
            instructions: self.instructions.unwrap_or_default(),
            model: self.model.ok_or("Agent model is required")?,
            tools: self.tools,
            handoffs: self.handoffs,
            input_guardrails: self.input_guardrails,
            output_guardrails: self.output_guardrails,
            hooks: self.hooks,
        })
    }
}

impl Default for AgentBuilder {
    fn default() -> Self {
        Self::new()
    }
}
