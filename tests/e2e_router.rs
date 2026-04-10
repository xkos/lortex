//! Integration test: Router as Provider → Runner 完整循环
//!
//! 验证 Router 作为 Provider 传入 Runner 后，能正确路由、执行、追踪成本。

use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use futures::Stream;
use serde_json::{json, Value};

use lortex::core::agent::AgentBuilder;
use lortex::core::error::ProviderError;
use lortex::core::message::{ContentPart, Message};
use lortex::core::provider::{
    CompletionRequest, CompletionResponse, Provider, ProviderCapabilities, StreamEvent, Usage,
};
use lortex::core::tool::{FnTool, Tool, ToolOutput};
use lortex::executor::Runner;
use lortex::router::{
    Capabilities, CostProfile, CostTracker, FixedRouter, Modality, ModelProfile, ModelRegistry,
    RouterBuilder,
};

// --- Mock Provider ---

struct MockLLM {
    name: String,
    responses: Vec<CompletionResponse>,
    call_count: AtomicUsize,
}

impl MockLLM {
    fn new(name: &str, responses: Vec<CompletionResponse>) -> Self {
        Self {
            name: name.into(),
            responses,
            call_count: AtomicUsize::new(0),
        }
    }

    fn calls(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait]
impl Provider for MockLLM {
    fn name(&self) -> &str {
        &self.name
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

fn text_response(text: &str) -> CompletionResponse {
    CompletionResponse {
        message: Message::assistant(text),
        usage: Some(Usage {
            prompt_tokens: 100,
            completion_tokens: 50,
            total_tokens: 150,
        }),
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
        usage: Some(Usage {
            prompt_tokens: 80,
            completion_tokens: 30,
            total_tokens: 110,
        }),
        finish_reason: None,
        model: "mock".into(),
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

/// Router → Runner: simple text response
#[tokio::test]
async fn router_runner_simple_text() {
    let llm = Arc::new(MockLLM::new("openai", vec![text_response("Hello from router!")]));

    let router = RouterBuilder::new()
        .provider("openai", llm.clone())
        .registry(test_registry())
        .strategy(FixedRouter::new("openai", "gpt-4o"))
        .build()
        .unwrap();

    let runner = Runner::new(Arc::new(router));
    let agent = AgentBuilder::new()
        .name("test")
        .instructions("Be helpful")
        .model("any")
        .build()
        .unwrap();

    let output = runner.run(&agent, "Hi").await.unwrap();
    assert_eq!(output.message.text(), Some("Hello from router!"));
    assert_eq!(llm.calls(), 1);
}

/// Router → Runner: tool call cycle with cost tracking
#[tokio::test]
async fn router_runner_tool_call_with_cost() {
    let calc: Arc<dyn Tool> = Arc::new(FnTool::new(
        "calc",
        "Calculate",
        json!({"type":"object","properties":{"expr":{"type":"string"}}}),
        |args| async move {
            let expr = args.get("expr").and_then(|v| v.as_str()).unwrap_or("?");
            Ok(ToolOutput::text(format!("{expr} = 42")))
        },
    ));

    let llm = Arc::new(MockLLM::new(
        "openai",
        vec![
            tool_call_response("calc", "c1", json!({"expr": "6*7"})),
            text_response("The answer is 42."),
        ],
    ));

    let tracker = Arc::new(CostTracker::new());

    let router = RouterBuilder::new()
        .provider("openai", llm.clone())
        .registry(test_registry())
        .strategy(FixedRouter::new("openai", "gpt-4o"))
        .cost_tracker(tracker.clone())
        .build()
        .unwrap();

    let runner = Runner::new(Arc::new(router));
    let agent = AgentBuilder::new()
        .name("math")
        .instructions("Use calc tool")
        .model("any")
        .tool(calc)
        .build()
        .unwrap();

    let output = runner.run(&agent, "What is 6*7?").await.unwrap();
    assert_eq!(output.message.text(), Some("The answer is 42."));

    // Two LLM calls → two cost records
    assert_eq!(llm.calls(), 2);
    assert!(tracker.total_cost() > 0.0);
    let records = tracker.records().await;
    assert_eq!(records.len(), 2);
    assert!(records.iter().all(|r| r.provider == "openai" && r.model == "gpt-4o"));

    let (input, output_tokens) = tracker.total_tokens().await;
    assert_eq!(input, 180);  // 100 + 80
    assert_eq!(output_tokens, 80); // 50 + 30
}

/// Router → Runner: routing error propagates correctly
#[tokio::test]
async fn router_runner_routing_error() {
    let llm = Arc::new(MockLLM::new("openai", vec![text_response("hi")]));

    // Registry is empty — FixedRouter will fail to find the model
    let router = RouterBuilder::new()
        .provider("openai", llm)
        .registry(ModelRegistry::new())
        .strategy(FixedRouter::new("openai", "gpt-4o"))
        .build()
        .unwrap();

    let runner = Runner::new(Arc::new(router));
    let agent = AgentBuilder::new()
        .name("test")
        .instructions("")
        .model("any")
        .build()
        .unwrap();

    let result = runner.run(&agent, "test").await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Model not found"));
}
