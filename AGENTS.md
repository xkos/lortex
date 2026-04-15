# Lortex 框架

模块化、高性能的 Rust Agent 框架。

## 核心约束

## 子 crate 结构

```
lortex/
├── Cargo.toml    — workspace + facade crate
├── src/lib.rs    — 统一 re-export 所有子 crate
├── crates/
│   ├── core/         — 核心 trait 和类型（Agent, Tool, Provider, Memory, Message 等）
│   ├── executor/     — 执行引擎（Runner, ReAct, PlanAndExecute）
│   ├── providers/    — LLM 提供商（OpenAI, Anthropic, Gemini, DeepSeek, 本地模型）
│   ├── router/       — 异构模型路由（按质量/成本/隐私路由到不同模型）
│   ├── protocols/    — Agent 协议（MCP, A2A）
│   ├── tools/        — 内置工具 + 注册表
│   ├── swarm/        — 多 Agent 编排（Router, Pipeline, Parallel, Hierarchical）
│   ├── guardrails/   — 安全护栏（ContentFilter, RateLimiter, TokenBudget, ToolApproval）
│   ├── memory/       — 记忆实现（InMemory, SlidingWindow）
│   ├── server/       — 本地 LLM Proxy 服务（HTTP Gateway, 兼容 OpenAI API）
│   └── macros/       — proc-macro（#[tool] 宏）
├── examples/
└── docs/
```

## 依赖方向

core 是唯一公共依赖，子 crate 之间尽量不互相依赖：

```
lortex(facade) → 所有子 crate
server         → core, router, providers
executor       → core
providers      → core
router         → core
protocols      → core
tools          → core
swarm          → core, executor
guardrails     → core
memory         → core
macros         （proc-macro，无运行时依赖）
```

## 设计原则

- 模块化：每个能力独立成 crate，可按需引入
- Provider 无关：业务逻辑通过 trait 抽象，不绑定具体 LLM
- 异构优先：Router 是一等公民，支持按任务特征路由到不同模型
- 可嵌入：作为库使用，不强制运行时或框架

## 文档

- 需求：[docs/prds/overview.md](docs/prds/overview.md)
- 架构：[docs/tech/architecture.md](docs/tech/architecture.md)
