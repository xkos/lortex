//! Multi-Agent Orchestration Example
//!
//! Demonstrates:
//! - Creating multiple specialized agents
//! - Using Handoffs for agent-to-agent delegation
//! - Using the Orchestrator with Router pattern
//!
//! Usage:
//!   OPENAI_API_KEY=your-key cargo run --example multi-agent

use std::sync::Arc;
use lortex::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| {
        eprintln!("Warning: OPENAI_API_KEY not set. Using placeholder.");
        "sk-placeholder".to_string()
    });

    let provider = Arc::new(OpenAIProvider::new(api_key));

    // Create specialized agents
    let code_agent = Arc::new(
        AgentBuilder::new()
            .name("code-expert")
            .instructions(
                "你是一个代码专家。负责回答编程、代码审查、算法相关的问题。\
                 用简洁的中文回答，并在需要时提供代码示例。"
            )
            .model("gpt-4o")
            .tool(Arc::new(ReadFileTool) as Arc<dyn Tool>)
            .tool(Arc::new(ShellTool::new()) as Arc<dyn Tool>)
            .build()
            .expect("Failed to build code agent"),
    );

    let ops_agent = Arc::new(
        AgentBuilder::new()
            .name("ops-expert")
            .instructions(
                "你是一个运维专家。负责回答系统管理、部署、监控相关的问题。\
                 用简洁的中文回答。"
            )
            .model("gpt-4o")
            .tool(Arc::new(ShellTool::new().with_timeout(15)) as Arc<dyn Tool>)
            .tool(Arc::new(HttpTool::new()) as Arc<dyn Tool>)
            .build()
            .expect("Failed to build ops agent"),
    );

    // Create a triage agent that routes to specialists
    let triage_agent = Arc::new(
        AgentBuilder::new()
            .name("triage")
            .instructions(
                "你是一个分诊Agent。根据用户问题的类型，决定应该由哪个专家来处理：\n\
                 - 如果问题涉及编程、代码、算法，请转交给 code-expert\n\
                 - 如果问题涉及系统管理、运维、部署，请转交给 ops-expert\n\
                 - 如果无法确定，请自行回答"
            )
            .model("gpt-4o")
            .handoff_to(code_agent.clone() as Arc<dyn Agent>)
            .handoff_to(ops_agent.clone() as Arc<dyn Agent>)
            .build()
            .expect("Failed to build triage agent"),
    );

    // Build the runner
    let runner = Arc::new(
        Runner::builder()
            .provider(provider)
            .max_iterations(10)
            .build()
            .expect("Failed to build runner"),
    );

    // Create an orchestrator with Router pattern
    let orchestrator = Orchestrator::builder()
        .pattern(OrchestrationPattern::Router {
            triage_agent: triage_agent as Arc<dyn Agent>,
        })
        .runner(runner)
        .build()
        .expect("Failed to build orchestrator");

    println!("=== Multi-Agent Orchestration Example ===\n");

    // Example queries
    let queries = vec![
        "用Rust写一个快速排序函数",
        "如何查看Linux系统的内存使用情况",
    ];

    for query in queries {
        println!("User: {}\n", query);

        match orchestrator.run(query).await {
            Ok(output) => {
                println!(
                    "Response (from {}): {}\n",
                    output.agent_name,
                    output.message.text().unwrap_or("(no text)")
                );
            }
            Err(e) => {
                eprintln!("Error: {}\n", e);
            }
        }
        println!("---\n");
    }

    Ok(())
}
