//! Simple Agent Example
//!
//! Demonstrates the basic usage of agents:
//! - Creating an agent with tools
//! - Running the agent with the Runner
//! - Handling the output
//!
//! Usage:
//!   OPENAI_API_KEY=your-key cargo run --example simple-agent

use std::sync::Arc;
use lortex::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for observability
    tracing_subscriber::fmt::init();

    // Get API key from environment
    let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| {
        eprintln!("Warning: OPENAI_API_KEY not set. Using placeholder.");
        "sk-placeholder".to_string()
    });

    // Create built-in tools
    let read_file = Arc::new(ReadFileTool) as Arc<dyn Tool>;
    let shell = Arc::new(ShellTool::new().with_timeout(10)) as Arc<dyn Tool>;
    let http = Arc::new(HttpTool::new()) as Arc<dyn Tool>;

    // Build an agent
    let agent = AgentBuilder::new()
        .name("assistant")
        .instructions(
            "你是一个通用助手。你可以读取文件、执行Shell命令和发送HTTP请求。\
             请用简洁清晰的中文回答用户问题。"
        )
        .model("gpt-4o")
        .tool(read_file)
        .tool(shell)
        .tool(http)
        .build()
        .expect("Failed to build agent");

    // Create a provider
    let provider = Arc::new(OpenAIProvider::new(api_key));

    // Build a runner
    let runner = Runner::builder()
        .provider(provider)
        .max_iterations(5)
        .build()
        .expect("Failed to build runner");

    // Run the agent
    println!("=== Simple Agent Example ===\n");
    println!("Agent: {}", agent.name());
    println!("Model: {}\n", agent.model());

    // Example: ask the agent a question
    let input = "读取当前目录下的 Cargo.toml 文件内容，告诉我这个项目的名称和版本。";
    println!("User: {}\n", input);

    match runner.run(&agent, input).await {
        Ok(output) => {
            println!("Assistant: {}", output.message.text().unwrap_or("(no text)"));
            println!("\n--- Run Info ---");
            println!("Agent: {}", output.agent_name);
            println!("Messages: {}", output.messages.len());
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }

    Ok(())
}
