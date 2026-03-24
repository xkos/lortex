//! Vibe Coding Example
//!
//! Demonstrates how to build a coding assistant agent that can:
//! - Read and write files
//! - Execute shell commands (compile, test, lint)
//! - Iterate on code changes using the ReAct loop
//!
//! Usage:
//!   OPENAI_API_KEY=your-key cargo run --example vibe-coding

use std::sync::Arc;
use lortex::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| {
        eprintln!("Warning: OPENAI_API_KEY not set. Using placeholder.");
        "sk-placeholder".to_string()
    });

    // Vibe coding tools
    let read_file = Arc::new(ReadFileTool) as Arc<dyn Tool>;
    let write_file = Arc::new(WriteFileTool) as Arc<dyn Tool>;
    let shell = Arc::new(
        ShellTool::new()
            .with_working_dir(".")
            .with_timeout(30),
    ) as Arc<dyn Tool>;

    // Content safety guardrail
    let content_filter = Arc::new(ContentFilter::new(vec![])) as Arc<dyn Guardrail>;

    // Token budget guardrail (100K tokens max per session)
    let token_budget = Arc::new(TokenBudget::new(100_000)) as Arc<dyn Guardrail>;

    // Build the coding agent
    let agent = AgentBuilder::new()
        .name("vibe-coder")
        .instructions(
            "你是一个高级Rust编程助手（Vibe Coder）。\n\n\
             你的工作流程：\n\
             1. 理解用户的编程需求\n\
             2. 读取相关源文件了解现有代码结构\n\
             3. 编写或修改代码\n\
             4. 使用Shell工具编译和测试代码\n\
             5. 如果编译失败，分析错误并修复\n\
             6. 重复以上步骤直到代码正确运行\n\n\
             规则：\n\
             - 编写惯用的Rust代码，遵循Rust最佳实践\n\
             - 始终在修改后运行 `cargo check` 验证编译\n\
             - 给出清晰的中文解释说明你做了什么改动"
        )
        .model("gpt-4o")
        .tool(read_file)
        .tool(write_file)
        .tool(shell)
        .input_guardrail(content_filter)
        .input_guardrail(token_budget)
        .build()
        .expect("Failed to build agent");

    // Provider and runner
    let provider = Arc::new(OpenAIProvider::new(api_key));
    let runner = Runner::builder()
        .provider(provider)
        .max_iterations(15) // More iterations for coding tasks
        .build()
        .expect("Failed to build runner");

    println!("=== Vibe Coding Agent Example ===\n");
    println!("Agent: {} (model: {})", agent.name(), agent.model());
    println!("Tools: read_file, write_file, shell\n");

    let task = "读取 Cargo.toml 文件，告诉我这个 workspace 包含了哪些 crate。";
    println!("User: {}\n", task);

    match runner.run(&agent, task).await {
        Ok(output) => {
            println!("Vibe Coder: {}", output.message.text().unwrap_or("(no text)"));
            println!("\n--- Session Stats ---");
            println!("Total messages exchanged: {}", output.messages.len());
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }

    Ok(())
}
