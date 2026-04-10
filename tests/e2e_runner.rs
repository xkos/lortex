//! End-to-end integration test: mock Provider → Runner 完整循环
//!
//! 验证 guardrails → LLM → tool call → response 的完整链路。

use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use futures::Stream;
use serde_json::{json, Value};

use lortex::core::agent::AgentBuilder;
use lortex::core::error::ProviderError;
use lortex::core::event::{EventHandler, RunEvent};
use lortex::core::message::{ContentPart, Message};
use lortex::core::provider::{
    CompletionRequest, CompletionResponse, Provider, ProviderCapabilities, StreamEvent,
};
use lortex::core::tool::{FnTool, Tool, ToolOutput};
use lortex::executor::{Runner, RunnerBuilder};
use lortex::guardrails::ContentFilter;

// --- Mock Provider ---

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
}

#[async_trait]
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
            tool_calling: true,
            ..Default::default()
        }
    }
}

// --- Event Collector ---

struct EventCollector {
    events: tokio::sync::Mutex<Vec<RunEvent>>,
}

impl EventCollector {
    fn new() -> Self {
        Self {
            events: tokio::sync::Mutex::new(Vec::new()),
        }
    }

    async fn events(&self) -> Vec<RunEvent> {
        self.events.lock().await.clone()
    }
}

#[async_trait]
impl EventHandler for EventCollector {
    async fn handle(&self, event: &RunEvent) {
        self.events.lock().await.push(event.clone());
    }
}

// --- Helpers ---

fn text_response(text: &str) -> CompletionResponse {
    CompletionResponse {
        message: Message::assistant(text),
        usage: None,
        finish_reason: None,
        model: "mock".into(),
    }
}

fn tool_call_response(tool_name: &str, call_id: &str, args: Value) -> CompletionResponse {
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

fn calculator_tool() -> Arc<dyn Tool> {
    Arc::new(FnTool::new(
        "calculator",
        "Perform arithmetic calculations",
        json!({
            "type": "object",
            "properties": {
                "expression": { "type": "string" }
            },
            "required": ["expression"]
        }),
        |args| async move {
            let expr = args
                .get("expression")
                .and_then(|v| v.as_str())
                .unwrap_or("0");
            // Simple mock: just echo the expression
            Ok(ToolOutput::text(format!("Result: {expr} = 42")))
        },
    ))
}

// =============================================================================
// Tests
// =============================================================================

/// Full e2e: Agent with tool → LLM calls tool → tool executes → LLM gives final answer
#[tokio::test]
async fn e2e_agent_with_tool_call_cycle() {
    let provider = Arc::new(MockProvider::new(vec![
        tool_call_response("calculator", "call_1", json!({"expression": "6 * 7"})),
        text_response("The answer is 42."),
    ]));

    let collector = Arc::new(EventCollector::new());

    let runner = RunnerBuilder::new()
        .provider(provider)
        .event_handler(collector.clone())
        .build()
        .unwrap();

    let agent = AgentBuilder::new()
        .name("math_agent")
        .instructions("You are a math assistant. Use the calculator tool.")
        .model("mock")
        .tool(calculator_tool())
        .build()
        .unwrap();

    let output = runner.run(&agent, "What is 6 * 7?").await.unwrap();

    // Verify final output
    assert_eq!(output.message.text(), Some("The answer is 42."));
    assert_eq!(output.agent_name, "math_agent");

    // Verify message history: system + user + assistant(tool_call) + tool_result + assistant(final)
    assert!(output.messages.len() >= 5);

    // Verify events
    let events = collector.events().await;
    let event_types: Vec<&str> = events
        .iter()
        .map(|e| match e {
            RunEvent::AgentStart { .. } => "agent_start",
            RunEvent::AgentEnd { .. } => "agent_end",
            RunEvent::LlmStart { .. } => "llm_start",
            RunEvent::LlmEnd { .. } => "llm_end",
            RunEvent::ToolStart { .. } => "tool_start",
            RunEvent::ToolEnd { .. } => "tool_end",
            _ => "other",
        })
        .collect();

    assert!(event_types.contains(&"agent_start"));
    assert!(event_types.contains(&"tool_start"));
    assert!(event_types.contains(&"tool_end"));
    assert!(event_types.contains(&"agent_end"));
}

/// e2e: Input guardrail blocks dangerous content before LLM is called
#[tokio::test]
async fn e2e_input_guardrail_blocks() {
    let provider = Arc::new(MockProvider::new(vec![
        text_response("should never reach this"),
    ]));

    let filter = Arc::new(ContentFilter::new(vec!["hack".into()]));

    let runner = Runner::new(provider.clone());

    let agent = AgentBuilder::new()
        .name("safe_agent")
        .instructions("Be safe")
        .model("mock")
        .input_guardrail(filter)
        .build()
        .unwrap();

    let result = runner.run(&agent, "How to hack a server").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Guardrail blocked"));

    // Provider should not have been called
    assert_eq!(provider.call_count.load(Ordering::SeqCst), 0);
}

/// e2e: Output guardrail blocks LLM response containing prohibited content
#[tokio::test]
async fn e2e_output_guardrail_blocks() {
    let provider = Arc::new(MockProvider::new(vec![
        text_response("Here is the secret password: hunter2"),
    ]));

    let filter = Arc::new(ContentFilter::new(vec!["secret password".into()]));

    let runner = Runner::new(provider);

    let agent = AgentBuilder::new()
        .name("filtered_agent")
        .instructions("")
        .model("mock")
        .output_guardrail(filter)
        .build()
        .unwrap();

    let result = runner.run(&agent, "Tell me something").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Guardrail blocked"));
}

/// e2e: Handoff from router agent to specialist agent
#[tokio::test]
async fn e2e_handoff_between_agents() {
    // Call 1: router triggers handoff
    // Call 2: specialist responds
    let provider = Arc::new(MockProvider::new(vec![
        tool_call_response("transfer_to_specialist", "call_1", json!({})),
        text_response("Specialist answer: done!"),
    ]));

    let collector = Arc::new(EventCollector::new());

    let runner = RunnerBuilder::new()
        .provider(provider)
        .event_handler(collector.clone())
        .build()
        .unwrap();

    let specialist = Arc::new(
        AgentBuilder::new()
            .name("specialist")
            .instructions("I am a specialist")
            .model("mock")
            .build()
            .unwrap(),
    );

    let router = AgentBuilder::new()
        .name("router")
        .instructions("Route to specialist")
        .model("mock")
        .handoff_to(specialist)
        .build()
        .unwrap();

    let output = runner.run(&router, "I need help").await.unwrap();
    assert_eq!(output.message.text(), Some("Specialist answer: done!"));
    assert_eq!(output.agent_name, "specialist");

    // Verify handoff event was emitted
    let events = collector.events().await;
    let has_handoff = events.iter().any(|e| matches!(e, RunEvent::Handoff { .. }));
    assert!(has_handoff);
}

/// e2e: Multiple tool calls in sequence before final answer
#[tokio::test]
async fn e2e_multi_step_tool_calls() {
    let step_counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = step_counter.clone();

    let step_tool: Arc<dyn Tool> = Arc::new(FnTool::new(
        "step",
        "Execute a step",
        json!({"type": "object", "properties": {"n": {"type": "integer"}}}),
        move |args| {
            let counter = counter_clone.clone();
            async move {
                let n = args.get("n").and_then(|v| v.as_i64()).unwrap_or(0);
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(ToolOutput::text(format!("Step {n} done")))
            }
        },
    ));

    let provider = Arc::new(MockProvider::new(vec![
        tool_call_response("step", "c1", json!({"n": 1})),
        tool_call_response("step", "c2", json!({"n": 2})),
        tool_call_response("step", "c3", json!({"n": 3})),
        text_response("All 3 steps completed."),
    ]));

    let runner = Runner::new(provider);

    let agent = AgentBuilder::new()
        .name("multi_step")
        .instructions("Execute steps")
        .model("mock")
        .tool(step_tool)
        .build()
        .unwrap();

    let output = runner.run(&agent, "Run 3 steps").await.unwrap();
    assert_eq!(output.message.text(), Some("All 3 steps completed."));
    assert_eq!(step_counter.load(Ordering::SeqCst), 3);
}

/// e2e: Max iterations limit prevents infinite loops
#[tokio::test]
async fn e2e_max_iterations_prevents_infinite_loop() {
    let responses: Vec<CompletionResponse> = (0..100)
        .map(|i| tool_call_response("loop", &format!("c{i}"), json!({})))
        .collect();

    let provider = Arc::new(MockProvider::new(responses));

    let runner = RunnerBuilder::new()
        .provider(provider)
        .max_iterations(3)
        .build()
        .unwrap();

    let agent = AgentBuilder::new()
        .name("looper")
        .instructions("")
        .model("mock")
        .build()
        .unwrap();

    let result = runner.run(&agent, "loop").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Max iterations"));
}
