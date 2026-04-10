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

#[cfg(test)]
mod tests {
    use super::*;
    use lortex_core::agent::{AgentBuilder, SimpleAgent};
    use lortex_core::error::ProviderError;
    use lortex_core::message::{ContentPart, Role};
    use lortex_core::provider::{
        CompletionRequest, CompletionResponse, ProviderCapabilities, StreamEvent,
    };
    use lortex_core::tool::{FnTool, ToolOutput};
    use std::sync::atomic::{AtomicUsize, Ordering};

    // --- Mock Provider ---

    /// A mock provider that returns pre-configured responses in sequence.
    struct MockProvider {
        responses: Vec<CompletionResponse>,
        call_count: AtomicUsize,
    }

    impl MockProvider {
        fn new(responses: Vec<CompletionResponse>) -> Self {
            Self {
                responses,
                call_count: AtomicUsize::new(0),
            }
        }

        /// Create a provider that always returns a simple text response.
        fn text(text: &str) -> Self {
            Self::new(vec![CompletionResponse {
                message: Message::assistant(text),
                usage: None,
                finish_reason: None,
                model: "mock".into(),
            }])
        }
    }

    #[async_trait::async_trait]
    impl Provider for MockProvider {
        fn name(&self) -> &str {
            "mock"
        }

        async fn complete(
            &self,
            _request: CompletionRequest,
        ) -> Result<CompletionResponse, ProviderError> {
            let idx = self.call_count.fetch_add(1, Ordering::SeqCst);
            self.responses
                .get(idx)
                .cloned()
                .ok_or_else(|| ProviderError::InvalidResponse("No more mock responses".into()))
        }

        fn complete_stream(
            &self,
            _request: CompletionRequest,
        ) -> Pin<Box<dyn Stream<Item = Result<StreamEvent, ProviderError>> + Send + '_>> {
            Box::pin(futures::stream::empty())
        }

        fn capabilities(&self) -> ProviderCapabilities {
            ProviderCapabilities {
                streaming: false,
                tool_calling: true,
                ..Default::default()
            }
        }
    }

    fn make_tool_call_response(tool_name: &str, call_id: &str, args: Value) -> CompletionResponse {
        let mut msg = Message::assistant("");
        msg.content = vec![ContentPart::ToolCall {
            id: call_id.into(),
            name: tool_name.into(),
            arguments: args,
        }];
        CompletionResponse {
            message: msg,
            usage: None,
            finish_reason: None,
            model: "mock".into(),
        }
    }

    fn make_text_response(text: &str) -> CompletionResponse {
        CompletionResponse {
            message: Message::assistant(text),
            usage: None,
            finish_reason: None,
            model: "mock".into(),
        }
    }

    fn simple_agent() -> SimpleAgent {
        AgentBuilder::new()
            .name("test_agent")
            .instructions("You are a test agent")
            .model("mock")
            .build()
            .unwrap()
    }

    // --- Tests ---

    #[test]
    fn runner_config_defaults() {
        let config = RunnerConfig::default();
        assert_eq!(config.max_iterations, 10);
        assert_eq!(config.max_tool_calls_per_turn, 20);
        assert!(config.enable_input_guardrails);
        assert!(config.enable_output_guardrails);
    }

    #[test]
    fn runner_builder_requires_provider() {
        let result = RunnerBuilder::new().build();
        assert!(result.is_err());
    }

    #[test]
    fn runner_builder_with_provider() {
        let provider = Arc::new(MockProvider::text("hi"));
        let runner = RunnerBuilder::new().provider(provider).build();
        assert!(runner.is_ok());
    }

    #[test]
    fn runner_builder_config_methods() {
        let provider = Arc::new(MockProvider::text("hi"));
        let runner = RunnerBuilder::new()
            .provider(provider)
            .max_iterations(5)
            .max_tool_calls_per_turn(3)
            .build()
            .unwrap();
        assert_eq!(runner.config.max_iterations, 5);
        assert_eq!(runner.config.max_tool_calls_per_turn, 3);
    }

    #[tokio::test]
    async fn run_simple_text_response() {
        let provider = Arc::new(MockProvider::text("Hello!"));
        let runner = Runner::new(provider);
        let agent = simple_agent();

        let output = runner.run(&agent, "Hi").await.unwrap();
        assert_eq!(output.message.text(), Some("Hello!"));
        assert_eq!(output.agent_name, "test_agent");
        // Messages should include: system + user + assistant
        assert!(output.messages.len() >= 3);
    }

    #[tokio::test]
    async fn run_with_tool_call() {
        // First response: tool call, second response: final text
        let echo_tool = Arc::new(FnTool::new(
            "echo",
            "Echo input",
            serde_json::json!({"type":"object","properties":{"text":{"type":"string"}}}),
            |args| async move {
                let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
                Ok(ToolOutput::text(format!("echoed: {text}")))
            },
        ));

        let provider = Arc::new(MockProvider::new(vec![
            make_tool_call_response("echo", "call_1", serde_json::json!({"text": "hello"})),
            make_text_response("Done echoing"),
        ]));

        let agent = AgentBuilder::new()
            .name("tool_agent")
            .instructions("Use tools")
            .model("mock")
            .tool(echo_tool)
            .build()
            .unwrap();

        let runner = Runner::new(provider);
        let output = runner.run(&agent, "echo something").await.unwrap();
        assert_eq!(output.message.text(), Some("Done echoing"));
    }

    #[tokio::test]
    async fn run_unknown_tool_returns_error_output() {
        // LLM calls a tool that doesn't exist, then gives final answer
        let provider = Arc::new(MockProvider::new(vec![
            make_tool_call_response("nonexistent", "call_1", serde_json::json!({})),
            make_text_response("Fallback answer"),
        ]));

        let agent = simple_agent();
        let runner = Runner::new(provider);
        let output = runner.run(&agent, "test").await.unwrap();
        assert_eq!(output.message.text(), Some("Fallback answer"));
        // The tool result message should contain an error
        let tool_result_msg = output
            .messages
            .iter()
            .find(|m| m.role == Role::Tool)
            .unwrap();
        match &tool_result_msg.content[0] {
            ContentPart::ToolResult { is_error, .. } => assert!(is_error),
            _ => panic!("expected ToolResult"),
        }
    }

    #[tokio::test]
    async fn run_max_iterations_exceeded() {
        // Provider always returns tool calls, never a final answer
        let responses: Vec<CompletionResponse> = (0..20)
            .map(|i| {
                make_tool_call_response("echo", &format!("call_{i}"), serde_json::json!({}))
            })
            .collect();

        let provider = Arc::new(MockProvider::new(responses));
        let runner = RunnerBuilder::new()
            .provider(provider)
            .max_iterations(3)
            .build()
            .unwrap();

        let agent = simple_agent();
        let result = runner.run(&agent, "loop forever").await;
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Max iterations (3) exceeded"));
    }

    #[tokio::test]
    async fn run_with_input_guardrail_block() {
        use lortex_core::guardrail::{Guardrail, GuardrailResult};

        struct BlockAll;

        #[async_trait::async_trait]
        impl Guardrail for BlockAll {
            fn name(&self) -> &str {
                "block_all"
            }
            async fn check_input(&self, _messages: &[Message]) -> GuardrailResult {
                GuardrailResult::Block {
                    message: "All input blocked".into(),
                }
            }
            async fn check_output(&self, _output: &Message) -> GuardrailResult {
                GuardrailResult::Pass
            }
        }

        let provider = Arc::new(MockProvider::text("should not reach"));
        let runner = Runner::new(provider);

        let agent = AgentBuilder::new()
            .name("guarded")
            .instructions("")
            .model("mock")
            .input_guardrail(Arc::new(BlockAll))
            .build()
            .unwrap();

        let result = runner.run(&agent, "blocked input").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Guardrail blocked"));
    }

    #[tokio::test]
    async fn run_with_output_guardrail_block() {
        use lortex_core::guardrail::{Guardrail, GuardrailResult};

        struct BlockOutput;

        #[async_trait::async_trait]
        impl Guardrail for BlockOutput {
            fn name(&self) -> &str {
                "block_output"
            }
            async fn check_input(&self, _messages: &[Message]) -> GuardrailResult {
                GuardrailResult::Pass
            }
            async fn check_output(&self, _output: &Message) -> GuardrailResult {
                GuardrailResult::Block {
                    message: "Output blocked".into(),
                }
            }
        }

        let provider = Arc::new(MockProvider::text("bad output"));
        let runner = Runner::new(provider);

        let agent = AgentBuilder::new()
            .name("guarded")
            .instructions("")
            .model("mock")
            .output_guardrail(Arc::new(BlockOutput))
            .build()
            .unwrap();

        let result = runner.run(&agent, "test").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Guardrail blocked"));
    }

    #[tokio::test]
    async fn run_with_messages_input() {
        let provider = Arc::new(MockProvider::text("response"));
        let runner = Runner::new(provider);
        let agent = simple_agent();

        let messages = vec![Message::user("hello"), Message::user("world")];
        let output = runner.run(&agent, messages).await.unwrap();
        assert_eq!(output.message.text(), Some("response"));
    }

    #[tokio::test]
    async fn run_with_handoff() {
        let target_agent = Arc::new(
            AgentBuilder::new()
                .name("target")
                .instructions("I am the target")
                .model("mock")
                .build()
                .unwrap(),
        );

        // First call: main agent triggers handoff
        // Second call: target agent responds
        let provider = Arc::new(MockProvider::new(vec![
            make_tool_call_response("transfer_to_target", "call_1", serde_json::json!({})),
            make_text_response("Target response"),
        ]));

        let agent = AgentBuilder::new()
            .name("router")
            .instructions("Route tasks")
            .model("mock")
            .handoff_to(target_agent)
            .build()
            .unwrap();

        let runner = Runner::new(provider);
        let output = runner.run(&agent, "do something").await.unwrap();
        assert_eq!(output.message.text(), Some("Target response"));
        assert_eq!(output.agent_name, "target");
    }

    #[tokio::test]
    async fn run_guardrails_disabled() {
        use lortex_core::guardrail::{Guardrail, GuardrailResult};

        struct AlwaysBlock;

        #[async_trait::async_trait]
        impl Guardrail for AlwaysBlock {
            fn name(&self) -> &str {
                "always_block"
            }
            async fn check_input(&self, _messages: &[Message]) -> GuardrailResult {
                GuardrailResult::Block {
                    message: "blocked".into(),
                }
            }
            async fn check_output(&self, _output: &Message) -> GuardrailResult {
                GuardrailResult::Block {
                    message: "blocked".into(),
                }
            }
        }

        let provider = Arc::new(MockProvider::text("success"));
        let config = RunnerConfig {
            enable_input_guardrails: false,
            enable_output_guardrails: false,
            ..Default::default()
        };
        let runner = RunnerBuilder::new()
            .provider(provider)
            .config(config)
            .build()
            .unwrap();

        let agent = AgentBuilder::new()
            .name("guarded")
            .instructions("")
            .model("mock")
            .input_guardrail(Arc::new(AlwaysBlock))
            .output_guardrail(Arc::new(AlwaysBlock))
            .build()
            .unwrap();

        // Should succeed because guardrails are disabled
        let output = runner.run(&agent, "test").await.unwrap();
        assert_eq!(output.message.text(), Some("success"));
    }
}
