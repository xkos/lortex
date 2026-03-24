//! lortex-executor: Agent 执行引擎
//!
//! 提供驱动 Agent 运行的核心组件：
//! - [`Runner`] — 执行主循环（guardrails → LLM 调用 → 工具执行 → handoff）
//! - [`ExecutionStrategy`] — 可插拔的执行策略（ReAct、Plan-and-Execute 等）

pub mod runner;
pub mod strategy;

pub use runner::{Runner, RunnerBuilder, RunnerConfig};
pub use strategy::{ExecutionStrategy, PlanAndExecuteStrategy, ReActStrategy};
