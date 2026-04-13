# Lortex 框架架构设计

> 关联需求：[Lortex PRD](../prds/overview.md)
> 最后更新：2026-03-03

---

## 一、设计决策汇总

| 决策项 | 选择 | 理由 |
|--------|------|------|
| 模块组织 | 子 workspace 多 crate | 用户可按需引入，编译可并行，适合未来开源 |
| 异构路由位置 | 独立 Router 模块 | 与 Provider 和 Executor 解耦，职责清晰 |
| 子 crate 命名 | 目录简洁名，package name 占位 | 开源时统一改名，内部用路径依赖不受影响 |
| core 是否拆分 | 不拆，保持一个 crate | 当前规模不大，拆分收益不明显，等需要时再拆 |

---

## 二、目录结构

```
rust/crates/lortex/
├── Cargo.toml                — lortex workspace 根配置
├── AGENTS.md                 — 框架级 AI 开发指南
├── docs/
│   ├── prds/overview.md      — 需求总览
│   └── tech/architecture.md  — 本文档
│
├── core/                     — 核心 trait 和类型
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── agent.rs          — Agent trait、SimpleAgent、AgentBuilder、Handoff
│       ├── tool.rs           — Tool trait、FnTool、ToolOutput
│       ├── provider.rs       — Provider trait、ProviderCapabilities
│       ├── memory.rs         — Memory trait、LayeredMemory
│       ├── guardrail.rs      — Guardrail trait、GuardrailResult
│       ├── message.rs        — Message、Role、ContentPart
│       ├── event.rs          — Event 枚举（AgentStart/End、LlmStart/End 等）
│       └── error.rs          — 类型化错误体系
│
├── executor/                 — 执行引擎
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── runner.rs         — Runner 主循环（guardrails → LLM → tools → handoff）
│       └── strategy.rs       — ExecutionStrategy（ReAct、PlanAndExecute）
│
├── providers/                — LLM 提供商
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── openai.rs         — OpenAI Provider
│       ├── anthropic.rs      — Anthropic Provider
│       ├── gemini.rs         — Google Gemini（Phase 2）
│       ├── deepseek.rs       — DeepSeek（Phase 2）
│       └── local.rs          — 本地模型 Ollama/llama.cpp（Phase 2）
│
├── router/                   — 异构模型路由（Phase 2）
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── registry.rs       — 模型注册与能力声明
│       ├── strategy.rs       — 路由策略（自动/手动/成本预算/回退）
│       └── cost.rs           — 成本追踪
│
├── protocols/                — Agent 协议
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── mcp/              — MCP 实现（client、server、types）
│       └── a2a/              — A2A 实现（client、types）
│
├── tools/                    — 内置工具 + 注册表
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── registry.rs       — ToolRegistry
│       └── builtin/          — 内置工具（file、http、shell）
│
├── swarm/                    — 多 Agent 编排
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── orchestrator.rs   — Orchestrator + Builder
│       └── patterns.rs       — Router、Pipeline、Parallel、Hierarchical
│
├── guardrails/               — 安全护栏实现
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── content_filter.rs
│       ├── rate_limiter.rs
│       ├── token_budget.rs
│       └── tool_approval.rs
│
├── memory/                   — 记忆实现
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── in_memory.rs
│       └── sliding_window.rs
│
├── server/                   — 本地 LLM Proxy 服务（Phase 2）
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── server.rs             — Axum HTTP 服务，暴露兼容 OpenAI API
│       └── config.rs             — 本地代理配置读取与热重载
│
├── macros/                   — proc-macro
│   ├── Cargo.toml
│   └── src/
│       └── lib.rs            — #[tool] 宏
│
└── lortex/                   — facade crate（统一入口）
    ├── Cargo.toml
    └── src/
        └── lib.rs            — re-export 所有子 crate 的公共 API
```

---

## 三、Crate 依赖关系

```
lortex（facade，re-export 全部）
  ├→ core
  ├→ server        → core, router, providers
  ├→ executor      → core
  ├→ providers     → core
  ├→ router        → core, providers
  ├→ protocols     → core
  ├→ tools         → core
  ├→ swarm         → core, executor
  ├→ guardrails    → core
  ├→ memory        → core
  └→ macros        （proc-macro，无运行时依赖）
```

关键约束：
- **core 是唯一的公共依赖**，所有子 crate 都依赖 core，但子 crate 之间尽量不互相依赖
- 例外：`executor` 被 `swarm` 依赖（编排需要执行引擎），`providers` 被 `router` 依赖（路由需要调用 Provider），`core`/`router`/`providers` 被 `server` 依赖提供代理能力
- **lortex facade** 依赖所有子 crate，通过 feature flag 控制哪些子 crate 被引入

---

## 四、核心架构：异构模型路由

```
Executor 需要调用 LLM
  │
  ▼
Router（智能 Provider）
  │
  ├→ 分析请求特征（任务复杂度、所需能力、上下文大小）
  ├→ 查询 ModelRegistry（已注册模型的能力、成本、速度）
  ├→ 应用路由策略：
  │    - 自动路由：匹配任务特征与模型能力
  │    - 手动指定：Agent 配置中指定了模型
  │    - 成本预算：在预算内选择最优模型
  │    - 回退：首选不可用时自动切换备选
  ├→ 选中 Provider + Model
  ├→ 调用 Provider.complete()
  ├→ 记录成本信息（token 消耗、价格）
  │
  ▼
返回结果 + CostRecord
```

### ModelRegistry

```rust
struct ModelProfile {
    provider: String,           // "openai", "anthropic", "local"
    model: String,              // "gpt-4o", "claude-opus", "deepseek-v3"
    capabilities: Capabilities, // 各维度评分
    cost: CostProfile,          // 输入/输出 token 价格
    speed: f32,                 // tokens/秒
    context_window: usize,      // 上下文窗口大小
    modalities: Vec<Modality>,  // 支持的模态
    supports_streaming: bool,
    supports_tools: bool,
}

struct Capabilities {
    planning: f32,     // 规划能力 0-1
    reasoning: f32,    // 推理能力
    coding: f32,       // 编码能力
    creative: f32,     // 创意能力
    simple_task: f32,  // 简单任务效率
}
```

### 路由策略

```rust
trait RoutingStrategy {
    fn select_model(
        &self,
        request: &RoutingRequest,
        registry: &ModelRegistry,
        budget: &CostBudget,
    ) -> Result<ModelSelection>;
}
```

内置策略：
- `AutoRouter` — 根据任务特征自动匹配
- `FixedRouter` — 固定使用指定模型
- `CostOptimizedRouter` — 在成本预算内优化质量
- `FallbackRouter` — 主模型 + 备选模型链

---

## 五、Executor 与 Router 的集成

Router 对 Executor 来说表现为一个实现了 `Provider` trait 的"智能 Provider"：

```rust
impl Provider for Router {
    async fn complete(&self, messages: &[Message], tools: &[&dyn Tool]) -> Result<Message> {
        let request = RoutingRequest::from_messages(messages, tools);
        let selection = self.strategy.select_model(&request, &self.registry, &self.budget)?;
        let provider = self.providers.get(&selection.provider)?;
        let result = provider.complete_with_model(&selection.model, messages, tools).await?;
        self.cost_tracker.record(&selection, &result);
        Ok(result)
    }
}
```

这样 Executor 不需要任何改动——它只知道自己在调用一个 Provider，不知道背后是单个模型还是路由器。

---

## 六、从现有代码到新结构的映射

| 现有 crate | 目标位置 | 变化 |
|------------|----------|------|
| taxon-core | lortex/core/ | 重命名，内容基本不变 |
| taxon-executor | lortex/executor/ | 重命名，内容基本不变 |
| taxon-providers | lortex/providers/ | 重命名，内容基本不变 |
| taxon-protocols | lortex/protocols/ | 重命名，内容基本不变 |
| taxon-tools | lortex/tools/ | 重命名，内容基本不变 |
| taxon-swarm | lortex/swarm/ | 重命名，内容基本不变 |
| taxon-guardrails | lortex/guardrails/ | 重命名，内容基本不变 |
| taxon-memory | lortex/memory/ | 重命名，内容基本不变 |
| taxon-macros | lortex/macros/ | 重命名，内容基本不变 |
| （新增） | lortex/router/ | Phase 2 新增，异构模型路由 |
| （新增） | lortex/server/ | Phase 2 新增，本地 LLM Proxy 代理服务 |
| lortex（旧） | lortex/lortex/ | 从空壳变为 facade，re-export 所有子 crate |

---

## 七、Phase 对应

| Phase | 涉及的子 crate | 说明 |
|-------|---------------|------|
| Phase 1 | core, executor, providers, protocols, tools, swarm, guardrails, memory, macros, lortex(facade) | 重组现有代码，统一结构 |
| Phase 2 | + router, + server, providers(扩展) | 异构模型路由、新增 Provider、本地 Proxy 服务 |
| Phase 3 | protocols(完善), + observability 能力（集成到各 crate 中） | MCP/A2A 完善、可观测性 |
| Phase 4 | memory(扩展), tools(扩展), swarm(扩展) | RAG、多模态、工作流、插件化 |

---

## 八、LLM Proxy 服务架构

`server` crate 提供了一个开箱即用的本地代理二进制模式：

```text
AI 代码软件 (Cursor/Windsurf)
  │
  ▼ [HTTP POST /v1/chat/completions (OpenAI 协议)]
  │
Server (Axum)
  │
  ├→ 协议解析 (将外界请求映射为 Lortex Message)
  ├→ 加载 proxy 路由配置 (API Keys & 路由规则)
  │
  ▼
Router (异构模型路由编排)
  │
  ├→ 匹配最优或指定的 Provider
  │
  ▼
Provider (OpenAI/Anthropic/DeepSeek/Local...)
```

**代理层职责边界**：
- `server` 仅做**协议暴露**、**密钥提取**和**流式兼容封包** (SSE -> OpenAI chunks)。
- 核心的**模型质量/成本评估**、**Token流控追踪**由底层的 `router` 和 `providers` 原生兜底。

---

## 九、开放问题
