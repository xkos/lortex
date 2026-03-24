//! Runner — the core execution engine that drives agent loops.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;

use futures::Stream;
use serde_json::Value;
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, warn};
use uuid::Uuid;

use lortex_core::agent::{Agent, Handoff, RunContext, RunInput, RunOutput};
use lortex_core::error::{AgentError, LortexError};
use lortex_core::event::{EventHandler, RunEvent};
use lortex_core::guardrail::GuardrailResult;
use lortex_core::message::Message;
use lortex_core::provider::{CompletionRequest, Provider, ToolDefinition};
use lortex_core::tool::{Tool, ToolContext, ToolOutput};

/// Configuration for the Runner.
#[derive(Debug, Clone)]
pub struct RunnerConfig {
    /// Maximum number of iterations in the agent loop.
    pub max_iterations: usize,

    /// Maximum number of tool calls per iteration.
    pub max_tool_calls_per_turn: usize,

    /// Whether to run input guardrails.
    pub enable_input_guardrails: bool,

    /// Whether to run output guardrails.
    pub enable_output_guardrails: bool,
}

impl Default for RunnerConfig {
    fn default() -> Self {
        Self {
            max_iterations: 10,
            max_tool_calls_per_turn: 20,
            enable_input_guardrails: true,
            enable_output_guardrails: true,
        }
    }
}

/// The Runner drives the agent execution loop.
pub struct Runner {
    /// The LLM provider.
    provider: Arc<dyn Provider>,

    /// Runner configuration.
    config: RunnerConfig,

    /// Event handlers for observability.
    event_handlers: Vec<Arc<dyn EventHandler>>,
}

impl Runner {
    /// Create a new Runner with a provider and default config.
    pub fn new(provider: Arc<dyn Provider>) -> Self {
        Self {
            provider,
            config: RunnerConfig::default(),
            event_handlers: vec![],
        }
    }

    /// Create a RunnerBuilder.
    pub fn builder() -> RunnerBuilder {
        RunnerBuilder::new()
    }

    /// Execute an agent with the given input (blocking until complete).
    pub async fn run(
        &self,
        agent: &dyn Agent,
        input: impl Into<RunInput>,
    ) -> Result<RunOutput, LortexError> {
        let input = input.into();
        let session_id = Uuid::new_v4().to_string();
        let mut context = RunContext::new(&session_id);

        // Convert input to messages
        let input_messages = match &input {
            RunInput::Text(text) => vec![Message::user(text.clone())],
            RunInput::Messages(msgs) => msgs.clone(),
        };

        // Add system message
        let instructions = agent.instructions();
        if !instructions.is_empty() {
            context.messages.push(Message::system(instructions));
        }
        context.messages.extend(input_messages);

        // Run input guardrails
        if self.config.enable_input_guardrails {
            self.check_input_guardrails(agent, &context.messages)
                .await?;
        }

        self.emit(RunEvent::AgentStart {
            agent: agent.name().to_string(),
        })
        .await;

        // Execute the agent loop
        let result = self.execute_loop(agent, &mut context).await;

        self.emit(RunEvent::AgentEnd {
            agent: agent.name().to_string(),
        })
        .await;

        result
    }

    /// Execute an agent with streaming events.
    pub fn run_stream(
        &self,
        agent: &dyn Agent,
        input: impl Into<RunInput>,
    ) -> Pin<Box<dyn Stream<Item = RunEvent> + Send + '_>> {
        let input = input.into();
        let (tx, rx) = tokio::sync::mpsc::channel(256);

        let agent_name = agent.name().to_string();
        let agent_instructions = agent.instructions().to_string();
        let agent_model = agent.model().to_string();
        let agent_tools = agent.tools();
        let agent_handoffs = agent.handoffs();
        let provider = self.provider.clone();
        tokio::spawn(async move {
            let _ = tx
                .send(RunEvent::AgentStart {
                    agent: agent_name.clone(),
                })
                .await;

            // Build messages
            let mut messages = vec![];
            if !agent_instructions.is_empty() {
                messages.push(Message::system(&agent_instructions));
            }
            match &input {
                RunInput::Text(text) => messages.push(Message::user(text.clone())),
                RunInput::Messages(msgs) => messages.extend(msgs.clone()),
            }

            // Build tool definitions
            let tool_defs = build_tool_definitions(&agent_tools, &agent_handoffs);

            let request = CompletionRequest {
                model: agent_model.clone(),
                messages: messages.clone(),
                tools: tool_defs,
                temperature: 0.7,
                max_tokens: None,
                stop: vec![],
                extra: Value::Null,
            };

            let _ = tx
                .send(RunEvent::LlmStart {
                    model: agent_model.clone(),
                    message_count: request.messages.len(),
                })
                .await;

            // Stream LLM response
            let mut stream = provider.complete_stream(request);
            use futures::StreamExt;
            while let Some(event) = stream.next().await {
                match event {
                    Ok(lortex_core::provider::StreamEvent::ContentDelta { delta }) => {
                        let _ = tx.send(RunEvent::LlmChunk { delta }).await;
                    }
                    Ok(lortex_core::provider::StreamEvent::Done { usage, .. }) => {
                        let _ = tx
                            .send(RunEvent::LlmEnd {
                                model: agent_model.clone(),
                                usage,
                            })
                            .await;
                    }
                    Err(e) => {
                        let _ = tx
                            .send(RunEvent::Error {
                                message: e.to_string(),
                            })
                            .await;
                    }
                    _ => {}
                }
            }

            let _ = tx
                .send(RunEvent::AgentEnd {
                    agent: agent_name,
                })
                .await;
        });

        Box::pin(ReceiverStream::new(rx))
    }

    // --- Private methods ---

    /// The core agent execution loop.
    fn execute_loop<'a>(
        &'a self,
        agent: &'a dyn Agent,
        context: &'a mut RunContext,
    ) -> Pin<Box<dyn std::future::Future<Output = Result<RunOutput, LortexError>> + Send + 'a>> {
        Box::pin(self.execute_loop_inner(agent, context))
    }

    async fn execute_loop_inner(
        &self,
        agent: &dyn Agent,
        context: &mut RunContext,
    ) -> Result<RunOutput, LortexError> {
        let tools = agent.tools();
        let handoffs = agent.handoffs();

        for iteration in 0..self.config.max_iterations {
            debug!(
                agent = agent.name(),
                iteration = iteration,
                "Agent loop iteration"
            );

            // Build tool definitions (tools + handoff tools)
            let tool_defs = build_tool_definitions(&tools, &handoffs);

            // Call the LLM
            let request = CompletionRequest {
                model: agent.model().to_string(),
                messages: context.messages.clone(),
                tools: tool_defs,
                temperature: 0.7,
                max_tokens: None,
                stop: vec![],
                extra: Value::Null,
            };

            self.emit(RunEvent::LlmStart {
                model: agent.model().to_string(),
                message_count: request.messages.len(),
            })
            .await;

            let response = self
                .provider
                .complete(request)
                .await
                .map_err(LortexError::Provider)?;

            self.emit(RunEvent::LlmEnd {
                model: response.model.clone(),
                usage: response.usage.clone(),
            })
            .await;

            // Add the assistant message to context
            context.messages.push(response.message.clone());

            // Invoke hooks
            if let Some(hooks) = agent.hooks() {
                hooks
                    .on_llm_response(agent.name(), &response.message)
                    .await;
            }

            // Check for tool calls
            let tool_calls = response.message.tool_calls();
            if tool_calls.is_empty() {
                // No tool calls — this is the final answer
                if self.config.enable_output_guardrails {
                    self.check_output_guardrails(agent, &response.message)
                        .await?;
                }

                return Ok(RunOutput {
                    message: response.message,
                    agent_name: agent.name().to_string(),
                    messages: context.messages.clone(),
                    metadata: HashMap::new(),
                });
            }

            // Process tool calls
            for (call_id, tool_name, args) in &tool_calls {
                // Check if this is a handoff
                if let Some(handoff) = handoffs.iter().find(|h| h.tool_name == *tool_name) {
                    self.emit(RunEvent::Handoff {
                        from: agent.name().to_string(),
                        to: handoff.target.name().to_string(),
                        reason: format!("LLM invoked handoff tool: {}", tool_name),
                    })
                    .await;

                    if let Some(hooks) = agent.hooks() {
                        hooks
                            .on_handoff(agent.name(), handoff.target.name())
                            .await;
                    }

                    // Filter messages for the target agent if a filter is specified
                    let handoff_messages = if let Some(filter) = &handoff.input_filter {
                        filter(&context.messages)
                    } else {
                        context.messages.clone()
                    };

                    // Recursively run the target agent
                    let mut target_context = RunContext::new(&context.session_id);
                    let target_instructions = handoff.target.instructions();
                    if !target_instructions.is_empty() {
                        target_context
                            .messages
                            .push(Message::system(target_instructions));
                    }
                    target_context.messages.extend(handoff_messages);

                    self.emit(RunEvent::AgentStart {
                        agent: handoff.target.name().to_string(),
                    })
                    .await;

                    let target_result = self
                        .execute_loop(&*handoff.target, &mut target_context)
                        .await?;

                    self.emit(RunEvent::AgentEnd {
                        agent: handoff.target.name().to_string(),
                    })
                    .await;

                    return Ok(target_result);
                }

                // Regular tool call
                self.emit(RunEvent::ToolStart {
                    name: tool_name.to_string(),
                    args: (*args).clone(),
                })
                .await;

                let tool_ctx = ToolContext {
                    session_id: context.session_id.clone(),
                    agent_name: agent.name().to_string(),
                    messages: context.messages.clone(),
                };

                let tool_result =
                    if let Some(tool) = tools.iter().find(|t| t.name() == *tool_name) {
                        if let Some(hooks) = agent.hooks() {
                            hooks.on_tool_start(agent.name(), tool_name, args).await;
                        }

                        match tool.execute((*args).clone(), &tool_ctx).await {
                            Ok(output) => {
                                if let Some(hooks) = agent.hooks() {
                                    hooks.on_tool_end(agent.name(), tool_name, &output).await;
                                }
                                output
                            }
                            Err(e) => {
                                warn!(tool = tool_name, error = %e, "Tool execution failed");
                                ToolOutput::error(e.to_string())
                            }
                        }
                    } else {
                        ToolOutput::error(format!("Tool not found: {}", tool_name))
                    };

                self.emit(RunEvent::ToolEnd {
                    name: tool_name.to_string(),
                    output: tool_result.content.clone(),
                    is_error: tool_result.is_error,
                })
                .await;

                // Add tool result to context
                context.messages.push(Message::tool_result(
                    *call_id,
                    tool_result.content,
                    tool_result.is_error,
                ));
            }
        }

        Err(LortexError::Agent(AgentError::MaxIterationsExceeded(
            self.config.max_iterations,
        )))
    }

    /// Check input guardrails.
    async fn check_input_guardrails(
        &self,
        agent: &dyn Agent,
        messages: &[Message],
    ) -> Result<(), LortexError> {
        for guardrail in agent.input_guardrails() {
            let result = guardrail.check_input(messages).await;
            self.emit(RunEvent::GuardrailTriggered {
                name: guardrail.name().to_string(),
                passed: result.is_pass(),
                message: match &result {
                    GuardrailResult::Warn { message } => Some(message.clone()),
                    GuardrailResult::Block { message } => Some(message.clone()),
                    _ => None,
                },
            })
            .await;

            if let GuardrailResult::Block { message } = result {
                return Err(LortexError::Agent(AgentError::GuardrailBlocked(message)));
            }
        }
        Ok(())
    }

    /// Check output guardrails.
    async fn check_output_guardrails(
        &self,
        agent: &dyn Agent,
        output: &Message,
    ) -> Result<(), LortexError> {
        for guardrail in agent.output_guardrails() {
            let result = guardrail.check_output(output).await;
            self.emit(RunEvent::GuardrailTriggered {
                name: guardrail.name().to_string(),
                passed: result.is_pass(),
                message: match &result {
                    GuardrailResult::Warn { message } => Some(message.clone()),
                    GuardrailResult::Block { message } => Some(message.clone()),
                    _ => None,
                },
            })
            .await;

            if let GuardrailResult::Block { message } = result {
                return Err(LortexError::Agent(AgentError::GuardrailBlocked(message)));
            }
        }
        Ok(())
    }

    /// Emit an event to all handlers.
    async fn emit(&self, event: RunEvent) {
        for handler in &self.event_handlers {
            handler.handle(&event).await;
        }
    }
}

/// Build tool definitions from tools and handoffs for the LLM request.
fn build_tool_definitions(
    tools: &[Arc<dyn Tool>],
    handoffs: &[Handoff],
) -> Vec<ToolDefinition> {
    let mut defs: Vec<ToolDefinition> = tools
        .iter()
        .map(|t| ToolDefinition {
            name: t.name().to_string(),
            description: t.description().to_string(),
            parameters: t.parameters_schema(),
        })
        .collect();

    // Add handoff tools
    for handoff in handoffs {
        defs.push(ToolDefinition {
            name: handoff.tool_name.clone(),
            description: handoff.tool_description.clone(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        });
    }

    defs
}

/// Builder for constructing a Runner.
pub struct RunnerBuilder {
    provider: Option<Arc<dyn Provider>>,
    config: RunnerConfig,
    event_handlers: Vec<Arc<dyn EventHandler>>,
}

impl RunnerBuilder {
    pub fn new() -> Self {
        Self {
            provider: None,
            config: RunnerConfig::default(),
            event_handlers: vec![],
        }
    }

    pub fn provider(mut self, provider: Arc<dyn Provider>) -> Self {
        self.provider = Some(provider);
        self
    }

    pub fn config(mut self, config: RunnerConfig) -> Self {
        self.config = config;
        self
    }

    pub fn max_iterations(mut self, max: usize) -> Self {
        self.config.max_iterations = max;
        self
    }

    pub fn max_tool_calls_per_turn(mut self, max: usize) -> Self {
        self.config.max_tool_calls_per_turn = max;
        self
    }

    pub fn event_handler(mut self, handler: Arc<dyn EventHandler>) -> Self {
        self.event_handlers.push(handler);
        self
    }

    pub fn build(self) -> Result<Runner, String> {
        Ok(Runner {
            provider: self.provider.ok_or("Provider is required")?,
            config: self.config,
            event_handlers: self.event_handlers,
        })
    }
}

impl Default for RunnerBuilder {
    fn default() -> Self {
        Self::new()
    }
}
