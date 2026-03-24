# Lortex 框架 · 产品需求总览

> 独立发展的 Rust Agent 框架，同时也是 Taxon AI 能力的基础。未来可能独立开源。
> 最后更新：2026-03-03

---

## 一、项目定位

一个**模块化、高性能的 Rust Agent 框架**，用于构建 AI 助手、自动化工具和智能代理。

核心差异化：
- **Rust 原生** — 高性能、内存安全、适合嵌入式和桌面场景
- **异构模型编排** — 任务拆分后可路由到不同质量/成本的模型，而非所有任务都用同一个 LLM
- **协议原生** — MCP、A2A 等 Agent 协议作为一等公民
- **可嵌入** — 可以作为库嵌入到其他 Rust 应用中（如 Taxon）

---

## 二、核心问题分析

| # | 痛点 | 本质 |
|---|------|------|
| 1 | 现有 Agent 框架大多是 Python 生态，Rust 生态缺乏成熟选择 | Rust Agent 框架空白 |
| 2 | 大多数 Agent 框架只支持单一 LLM 调用，所有任务用同一个模型 | 缺乏按任务质量/成本路由到不同模型的能力 |
| 3 | Agent 协议（MCP、A2A）在 Rust 中缺乏完整实现 | 协议生态不完善 |
| 4 | 工具扩展通常需要修改框架代码 | 缺乏插件化的工具体系 |
| 5 | Agent 运行过程不透明，难以调试和优化 | 缺乏可观测性 |

---

## 三、需求域划分

```
Lortex Framework
├── 域一：核心抽象（Agent, Tool, Provider, Memory, Message）
├── 域二：执行引擎（策略、循环、流式输出）
├── 域三：模型管理与异构编排
├── 域四：协议（MCP, A2A）
├── 域五：多 Agent 编排
├── 域六：安全与治理（Guardrails）
├── 域七：工具体系
├── 域八：记忆与上下文
├── 域九：可观测性
└── 域十：多模态与 RAG
```

---

### 域一：核心抽象

框架的基础类型和 trait 定义。

#### 需求 1.1 — Agent 抽象

- Agent 是声明式配置：system prompt、model、tools、handoffs、guardrails、hooks
- Agent 不负责执行（执行由 Runner 驱动）
- 支持 Builder 模式构建
- 支持 Handoff（Agent 间委托）

#### 需求 1.2 — Tool 抽象

- Tool 定义：name、description、JSON Schema 参数、execute 函数
- 支持 `#[tool]` 宏从普通函数生成 Tool
- 支持 `requires_approval()` 标记需要人工确认的危险操作
- FnTool 闭包封装，方便快速定义简单工具

#### 需求 1.3 — Provider 抽象

- LLM 统一接口：complete、complete_stream、embed、capabilities
- 支持多种 LLM 后端（详见域三）
- Provider 可声明自己的能力（支持的模型、是否支持流式、是否支持 embedding 等）

#### 需求 1.4 — Message 模型

- 统一消息格式：Role（System/User/Assistant/Tool）
- 多部分内容：Text、Image、ToolCall、ToolResult
- 元数据支持（任意键值对）
- 时间戳

#### 需求 1.5 — Error 体系

- 类型化错误：Agent/Tool/Provider/Memory/Guardrail 各有子错误类型
- 使用 thiserror 定义

---

### 域二：执行引擎

驱动 Agent 运行的核心引擎。

#### 需求 2.1 — Runner

- 执行主循环：输入 guardrails → LLM 调用 → tool 调用 → 输出 guardrails
- 支持 handoff 递归执行
- 支持阻塞执行（run）和流式事件（run_stream）
- 可配置：max_iterations、max_tool_calls_per_turn、guardrails 开关

#### 需求 2.2 — 执行策略

- **ReAct**（默认）：观察-思考-行动循环
- **Plan-and-Execute**：先规划完整计划，再逐步执行
- 策略可扩展，用户可自定义

#### 需求 2.3 — 事件系统

- 运行期事件：AgentStart/End、LlmStart/Chunk/End、ToolStart/End、Handoff、GuardrailTriggered
- 事件可用于日志、UI 更新、监控等

---

### 域三：模型管理与异构编排

**这是框架的核心差异化能力。**

#### 需求 3.1 — 多 Provider 支持

- OpenAI（GPT 系列）— 已实现
- Anthropic（Claude 系列）— 已实现
- Google Gemini
- DeepSeek
- 本地模型（通过 Ollama / llama.cpp / ONNX Runtime）
- 兼容 OpenAI API 格式的第三方服务

#### 需求 3.2 — 模型注册与能力声明

- 每个模型注册时声明自己的属性：
  - 能力等级（规划/推理/编码/简单任务等维度的评分）
  - 成本（每百万 token 的价格）
  - 速度（tokens/秒）
  - 支持的模态（文本/图片/视频/音频）
  - 上下文窗口大小
  - 是否支持流式、function calling、embedding

#### 需求 3.3 — 异构模型路由

任务拆分后，根据子任务的特征自动或手动路由到最合适的模型：

```
复杂任务输入
  → Plan Agent（高质量模型，如 Claude Opus）拆分为子任务
  → 子任务 A（需要深度推理）→ 高质量模型
  → 子任务 B（简单格式化）→ 低成本模型（如 GPT-4o-mini）
  → 子任务 C（隐私敏感）→ 本地模型
  → 汇总结果
```

路由策略：
- **自动路由**：根据子任务描述和模型能力声明，自动选择最优模型
- **手动指定**：用户可以为特定 Agent 或任务类型指定模型
- **成本预算**：设定总成本上限，路由器在预算内优化质量
- **回退机制**：首选模型不可用时，自动回退到备选模型

#### 需求 3.4 — 成本追踪

- 实时追踪每次 LLM 调用的 token 消耗和成本
- 按 Agent、按任务、按模型维度汇总
- 支持成本预算告警

---

### 域四：协议

Agent 间通信和工具发现的标准协议。

#### 需求 4.1 — MCP（Model Context Protocol）

- 完整的 MCP 实现
- Transport：SSE（已实现）、Stdio（待完成）
- Server：将本地 tools 暴露为 MCP 服务
- Client：连接外部 MCP 服务，发现并调用远程工具
- McpRemoteTool：将远程 MCP 工具包装为本地 Tool

#### 需求 4.2 — A2A（Agent-to-Agent）

- 完整的 A2A 实现
- AgentCard 发现机制
- 任务发送、状态查询、取消
- 与 MCP 互补：MCP 用于工具发现，A2A 用于 Agent 间协作

---

### 域五：多 Agent 编排

多个 Agent 协作完成复杂任务。

#### 需求 5.1 — 编排模式

- **Router**：分诊 Agent 根据任务类型路由到专家 Agent — 已实现
- **Pipeline**：按阶段顺序执行，前一阶段输出作为下一阶段输入 — 已实现
- **Parallel**：多 Agent 并行执行，由 aggregator 汇总 — 已实现
- **Hierarchical**：supervisor 协调 workers — 已实现
- **工作流引擎**（新增）：支持条件分支、循环、错误重试、人工介入等复杂流程

#### 需求 5.2 — Handoff

- Agent 间委托：当前 Agent 将任务交给另一个 Agent
- 支持上下文传递
- 支持递归 handoff

---

### 域六：安全与治理

#### 需求 6.1 — Guardrails

- **ContentFilter**：按关键词/模式过滤危险内容 — 已实现
- **RateLimiter**：调用频率限制 — 已实现
- **TokenBudget**：token 消耗预算 — 已实现
- **ToolApproval**：危险工具需人工确认 — 已实现
- 支持 Parallel（不阻塞）和 Blocking（阻塞）两种模式
- 可扩展：用户可自定义 Guardrail

---

### 域七：工具体系

#### 需求 7.1 — 内置工具

- ReadFileTool — 已实现
- WriteFileTool — 已实现
- HttpTool — 已实现
- ShellTool — 已实现

#### 需求 7.2 — 工具注册表

- 按名称注册、查找、移除工具 — 已实现

#### 需求 7.3 — 插件化工具体系

- 第三方可以开发工具插件
- 工具插件可以通过 crate 依赖引入
- 工具插件可以通过 MCP 协议远程加载
- `#[tool]` 宏简化工具开发 — 已实现

---

### 域八：记忆与上下文

#### 需求 8.1 — 会话记忆

- InMemoryStore：全量存储 — 已实现
- SlidingWindowMemory：保留最近 N 条 — 已实现
- SummaryMemory：超出窗口的内容自动摘要（待实现）

#### 需求 8.2 — 长期记忆

- 向量存储支持（RAG）：将历史对话和知识存入向量库，检索相关上下文
- 与 Taxon 的 LanceDB 可以共享基础设施

#### 需求 8.3 — 上下文管理

- 自动管理上下文窗口：当消息超出模型上下文限制时，智能截断或摘要
- 上下文优先级：system prompt > 最近消息 > 工具结果 > 历史消息

---

### 域九：可观测性

#### 需求 9.1 — Tracing

- 基于 `tracing` crate 的结构化日志
- 每次 Agent 运行生成完整的 trace（span 层级：Run → Agent → LLM Call → Tool Call）
- 支持导出到 OpenTelemetry

#### 需求 9.2 — Metrics

- token 消耗（按模型、按 Agent）
- 延迟（LLM 调用、工具调用、总执行时间）
- 成本（按模型定价计算）
- 成功/失败率

#### 需求 9.3 — 调试支持

- 可回放的执行记录（输入、每步 LLM 响应、工具调用结果、最终输出）
- 支持 dry-run 模式（不实际调用 LLM，用 mock 响应测试流程）

---

### 域十：多模态与 RAG

#### 需求 10.1 — 多模态输入

- 图片理解（已有 ContentPart::Image 定义，需要各 Provider 实现）
- 视频理解（提取关键帧后作为图片序列输入）
- 音频理解（转文字后输入，或直接支持音频模态的模型）

#### 需求 10.2 — RAG

- 文档加载：从文件/URL 加载文档，分块
- 向量化：调用 embedding 模型生成向量
- 检索：从向量库中检索相关文档块
- 注入：将检索结果注入到 Agent 的上下文中
- 与 Taxon 的 LanceDB 可以共享基础设施

---

## 四、优先级与演进路线

### Phase 1 — 基础能力

> 目标：核心抽象、执行引擎、基础工具链可用

| 需求 | 说明 |
|------|------|
| 1.1-1.5 核心抽象 | Agent、Tool、Provider、Memory、Message、Error 体系 |
| 2.1-2.3 执行引擎 | Runner、ReAct/Plan-and-Execute 策略、事件系统 |
| 5.1-5.2 多 Agent 编排 | Router、Pipeline、Parallel、Hierarchical、Handoff |
| 6.1 Guardrails | ContentFilter、RateLimiter、TokenBudget、ToolApproval |
| 7.1-7.2 工具体系 | 内置工具（File/HTTP/Shell）、工具注册表 |
| 8.1 会话记忆 | InMemory、SlidingWindow |
| 4.1-4.2 协议 | MCP（SSE + Stdio）、A2A 基础实现 |

### Phase 2 — 异构模型编排

> 目标：实现核心差异化能力

| 需求 | 说明 |
|------|------|
| 3.1 多 Provider | Gemini、DeepSeek、本地模型（Ollama/llama.cpp）等 |
| 3.2 模型注册 | 模型能力声明与注册机制 |
| 3.3 异构路由 | 按质量/成本/隐私路由到不同模型 |
| 3.4 成本追踪 | 实时 token 和成本追踪 |

### Phase 3 — 协议完善与可观测性

> 目标：完善协议实现，增加可观测性

| 需求 | 说明 |
|------|------|
| 4.1 MCP 完整实现 | 完整功能覆盖 |
| 4.2 A2A 完整实现 | 完善 Agent 间协作 |
| 9.1-9.3 可观测性 | tracing、metrics、调试支持 |

### Phase 4 — 高级能力

> 目标：多模态、RAG、工作流

| 需求 | 说明 |
|------|------|
| 10.1 多模态 | 图片/视频/音频理解 |
| 10.2 RAG | 文档加载、向量检索、上下文注入 |
| 5.1 工作流引擎 | 条件分支、循环、错误重试、人工介入 |
| 7.3 插件化工具 | 第三方工具插件体系 |
| 8.2-8.3 高级记忆 | SummaryMemory、长期记忆、上下文管理 |

---

## 五、核心设计原则

1. **模块化** — 每个能力独立成模块，可按需引入
2. **Provider 无关** — 业务逻辑不绑定具体 LLM，通过 trait 抽象
3. **异构优先** — 不假设所有任务用同一个模型，路由是一等公民
4. **协议原生** — MCP、A2A 不是附加功能，而是核心能力
5. **可嵌入** — 作为库使用，不强制运行时或框架
6. **可观测** — 每个操作都有 trace，成本和性能可追踪
7. **独立性** — 不依赖 Taxon 业务概念，可独立使用和开源

---

## 六、与 Taxon 的集成方式

lortex 框架通过 Taxon 自定义的 Tool 实现来控制平台：

```
用户自然语言指令
  → Agent 理解意图（可能用高质量模型）
  → Agent 拆分子任务
  → 子任务路由到合适的模型
  → Agent 选择 Taxon Tool 执行
      - TaxonSearchTool → 调用 taxon service::search
      - TaxonTagTool → 调用 taxon service::tag
      - TaxonImportTool → 调用 taxon service::import
      - ...
  → 返回结果
```

Tool 的接口定义在 lortex 框架中（`Tool` trait），Tool 的 Taxon 业务实现在 taxon 主 crate 中。这样 lortex 不依赖 Taxon，Taxon 依赖 lortex。

---

## 七、关键设计决策

### 7.1 异构路由的自动化

采用混合策略，用户可选择启用或不启用异构路由：

- 有明确规则匹配的（如 system prompt 关键词）→ 直接路由（零成本）
- 规则匹配不上的 → 用最便宜的模型做一次快速分类
- 用户手动指定的 → 最高优先级
- 不启用路由时 → 所有请求走 Agent 配置的默认模型

异构路由本身的成本（分类调用的 token 消耗）需要在 metrics 中单独记录，与正式调用的成本分开统计，方便用户对比"用路由 vs 不用路由"的总成本差异。

### 7.2 工作流引擎

当前不做通用工作流引擎。现有编排模式（Router + Pipeline + Parallel + Hierarchical + Handoff）已覆盖大部分场景。复杂流程由业务层用普通代码编排。

Phase 4 可考虑增加轻量 DAG 引擎（支持条件分支和并行，不支持循环和长时间运行）。

### 7.3 RAG 的边界

框架只定义接口和上下文注入机制，不内置具体的文档加载、分块、向量存储实现：

- **框架负责**：`Retriever` trait 定义、检索结果到上下文的注入、embedding 调用（通过 Provider.embed）
- **用户负责**：文档加载（PDF/Word/HTML 等）、分块策略、向量存储后端选择（LanceDB/Qdrant/Chroma 等）、`Retriever` trait 的具体实现

---

## 八、开放问题

1. **命名** — 已确定框架名为 Lortex，crate 前缀为 `lortex-`
