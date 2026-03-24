# core — 核心 trait 和类型

## 职责

定义 lortex 框架的所有核心抽象。所有其他子 crate 都依赖 core。

## 不做什么

- 不包含任何具体实现（实现在 executor、providers、memory 等子 crate 中）
- 不依赖任何其他 lortex 子 crate

## 核心类型

- **Agent**（agent.rs）— 声明式配置：system prompt、model、tools、handoffs、guardrails、hooks。包含 SimpleAgent、AgentBuilder、Handoff、AgentHooks
- **Tool**（tool.rs）— 外部能力接口：name、description、JSON Schema、execute。包含 FnTool 闭包封装、ToolOutput
- **Provider**（provider.rs）— LLM 统一接口：complete、complete_stream、embed、capabilities
- **Memory**（memory.rs）— 会话存储与检索：store、get、search、clear。包含 LayeredMemory
- **Guardrail**（guardrail.rs）— 输入/输出校验：Pass/Warn/Block，Parallel/Blocking 模式
- **Message**（message.rs）— 统一消息格式：Role、ContentPart（Text/Image/ToolCall/ToolResult）
- **Event**（event.rs）— 运行期事件枚举
- **Error**（error.rs）— 类型化错误体系

## 设计约束

- 所有 trait 必须是 `Send + Sync`（支持异步和多线程）
- 所有数据类型 derive `Serialize`/`Deserialize`（支持序列化）
- trait 方法使用 `async_trait`
- 错误类型使用 `thiserror`
