//! lortex — 模块化、高性能的 Rust Agent 框架
//!
//! 这是框架的统一入口 crate，re-export 所有子 crate 的公共 API。
//! 用户只需依赖此 crate 即可使用全部功能，也可以通过 feature flag 按需引入。
//!
//! # 子模块
//!
//! - [`core`] — 核心 trait 和类型（Agent、Tool、Provider、Memory、Message 等）
//! - [`executor`] — 执行引擎（Runner、执行策略）
//! - [`providers`] — LLM 提供商（OpenAI、Anthropic 等）
//! - [`protocols`] — Agent 协议（MCP、A2A）
//! - [`tools`] — 内置工具集与注册表
//! - [`swarm`] — 多 Agent 编排
//! - [`guardrails`] — 安全护栏
//! - [`memory`] — 记忆实现
//! - [`macros`] — 过程宏（#[tool]）

/// 核心 trait 和类型（始终可用）
pub use lortex_core as core;

/// 执行引擎
#[cfg(feature = "executor")]
pub use lortex_executor as executor;

/// LLM 提供商
#[cfg(feature = "providers")]
pub use lortex_providers as providers;

/// Agent 协议（MCP、A2A）
#[cfg(feature = "protocols")]
pub use lortex_protocols as protocols;

/// 内置工具集与注册表
#[cfg(feature = "tools")]
pub use lortex_tools as tools;

/// 多 Agent 编排
#[cfg(feature = "swarm")]
pub use lortex_swarm as swarm;

/// 安全护栏
#[cfg(feature = "guardrails")]
pub use lortex_guardrails as guardrails;

/// 记忆实现
#[cfg(feature = "memory")]
pub use lortex_memory as memory;

/// 异构模型路由
#[cfg(feature = "router")]
pub use lortex_router as router;

// 常用类型的便捷 re-export
pub use lortex_core::agent::{Agent, SimpleAgent, AgentBuilder};
pub use lortex_core::message::{Message, Role, ContentPart};
pub use lortex_core::tool::{Tool, ToolOutput};
pub use lortex_core::provider::Provider;
pub use lortex_core::error::LortexError;

/// 预导入模块 — 包含最常用的类型，方便 `use lortex::prelude::*`
pub mod prelude {
    // 核心 trait 和类型
    pub use lortex_core::agent::{Agent, SimpleAgent, AgentBuilder, Handoff};
    pub use lortex_core::message::{Message, Role, ContentPart};
    pub use lortex_core::tool::{Tool, ToolOutput};
    pub use lortex_core::provider::Provider;
    pub use lortex_core::guardrail::Guardrail;
    pub use lortex_core::memory::Memory;
    pub use lortex_core::error::LortexError;
    pub use lortex_core::event::RunEvent;

    // 执行引擎
    #[cfg(feature = "executor")]
    pub use lortex_executor::{Runner, RunnerBuilder, RunnerConfig};

    // LLM 提供商
    #[cfg(feature = "providers")]
    pub use lortex_providers::openai::OpenAIProvider;
    #[cfg(feature = "providers")]
    pub use lortex_providers::anthropic::AnthropicProvider;

    // 内置工具
    #[cfg(feature = "tools")]
    pub use lortex_tools::{ReadFileTool, WriteFileTool, ShellTool, HttpTool, ToolRegistry};

    // 多 Agent 编排
    #[cfg(feature = "swarm")]
    pub use lortex_swarm::{Orchestrator, OrchestratorBuilder, OrchestrationPattern};

    // 安全护栏
    #[cfg(feature = "guardrails")]
    pub use lortex_guardrails::{ContentFilter, RateLimiter, TokenBudget, ToolApproval};

    // 记忆
    #[cfg(feature = "memory")]
    pub use lortex_memory::{InMemoryStore, SlidingWindowMemory};

    // 路由
    #[cfg(feature = "router")]
    pub use lortex_router::{Router, RouterBuilder, FixedRouter, ModelRegistry, ModelProfile};

    // 常用外部类型
    pub use std::sync::Arc;
}
