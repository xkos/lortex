# guardrails — 安全护栏

## 职责

实现 core 中定义的 `Guardrail` trait，提供各种安全和限制机制。

## 依赖

- `core` — Guardrail trait、GuardrailResult、Message

## 已有实现

- **ContentFilter**（content_filter.rs）— 按关键词/短语过滤危险内容（大小写不敏感）
- **RateLimiter**（rate_limiter.rs）— 每分钟调用次数限制，80% 时 Warn
- **TokenBudget**（token_budget.rs）— 按约 4 字符/token 估算，限制总 token 消耗
- **ToolApproval**（tool_approval.rs）— 对指定工具在输出阶段拦截，要求人工审批

## 编码规范

- 每个 Guardrail 实现一个文件
- Guardrail 支持 Parallel（不阻塞执行）和 Blocking（阻塞等待）两种模式
- 用户可自定义 Guardrail，实现 core 的 Guardrail trait 即可
