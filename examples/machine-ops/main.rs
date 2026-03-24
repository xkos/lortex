//! Machine Operations Example
//!
//! Demonstrates how to build a machine management agent that can:
//! - Execute system commands for monitoring
//! - Make HTTP requests for API interactions
//! - Use guardrails for safety (approval required for dangerous operations)
//! - Use rate limiting to prevent abuse
//!
//! Usage:
//!   OPENAI_API_KEY=your-key cargo run --example machine-ops

use std::sync::Arc;
use lortex::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let api_key = std::env::var("OPENAI_API_KEY").unwrap_or_else(|_| {
        eprintln!("Warning: OPENAI_API_KEY not set. Using placeholder.");
        "sk-placeholder".to_string()
    });

    // Tools for machine management
    let shell = Arc::new(
        ShellTool::new()
            .with_timeout(30),
    ) as Arc<dyn Tool>;

    let http = Arc::new(
        HttpTool::new()
            .with_timeout(15),
    ) as Arc<dyn Tool>;

    let read_file = Arc::new(ReadFileTool) as Arc<dyn Tool>;

    // Safety guardrails
    // 1. Tool approval: require confirmation for dangerous operations
    let tool_approval = Arc::new(
        ToolApproval::new(vec!["write_file".to_string()])
    ) as Arc<dyn Guardrail>;

    // 2. Rate limiter: max 30 calls per minute
    let rate_limiter = Arc::new(RateLimiter::new(30)) as Arc<dyn Guardrail>;

    // 3. Content filter
    let content_filter = Arc::new(
        ContentFilter::new(vec![
            "rm -rf /".to_string(),
            "mkfs".to_string(),
            "dd if=".to_string(),
        ])
    ) as Arc<dyn Guardrail>;

    // Build the ops agent
    let agent = AgentBuilder::new()
        .name("ops-agent")
        .instructions(
            "你是一个机器管理Agent。你的职责是帮助用户管理和监控服务器。\n\n\
             你可以执行的操作：\n\
             - 查看系统状态（CPU、内存、磁盘、网络）\n\
             - 查看进程信息\n\
             - 读取配置文件和日志\n\
             - 调用API检查服务状态\n\n\
             安全规则：\n\
             - 永远不要执行破坏性操作（如删除系统文件、格式化磁盘）\n\
             - 只执行只读查询命令\n\
             - 如果用户请求危险操作，礼貌地拒绝并解释原因\n\n\
             请用清晰的中文回答，结果用表格或列表格式呈现。"
        )
        .model("gpt-4o")
        .tool(shell)
        .tool(http)
        .tool(read_file)
        .input_guardrail(content_filter)
        .input_guardrail(rate_limiter)
        .output_guardrail(tool_approval)
        .build()
        .expect("Failed to build agent");

    // Provider and runner
    let provider = Arc::new(OpenAIProvider::new(api_key));
    let runner = Runner::builder()
        .provider(provider)
        .max_iterations(8)
        .build()
        .expect("Failed to build runner");

    println!("=== Machine Operations Agent Example ===\n");
    println!("Agent: {} (model: {})", agent.name(), agent.model());
    println!("Guardrails: content_filter, rate_limiter, tool_approval\n");

    let tasks = vec![
        "查看当前系统的内存使用情况",
        "列出当前目录下所有的文件",
    ];

    for task in tasks {
        println!("User: {}\n", task);

        match runner.run(&agent, task).await {
            Ok(output) => {
                println!(
                    "Ops Agent: {}\n",
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
