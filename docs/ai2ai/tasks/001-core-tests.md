# 任务 001: 核心 crate 单元测试 + 端到端集成测试

> 状态：✅ 已关闭
> 分支：iter/001-core-tests
> 配对迭代：[iterations/001-core-tests.md](../iterations/001-core-tests.md)

## 迭代目标
为 executor、memory、guardrails、tools 四个 crate 补充单元测试，并新增一个最小端到端集成测试验证 Runner 完整循环。

## 验收标准（人审核/补充）
- executor crate 有覆盖 Runner 主循环和执行策略的单元测试
- memory crate 有覆盖 InMemory 和 SlidingWindow 的单元测试
- guardrails crate 有覆盖全部 4 个 guardrail 实现的单元测试
- tools crate 有覆盖 ToolRegistry 和内置工具的单元测试
- 有一个 e2e 集成测试：mock Provider → Runner 完整循环（guardrails → LLM → tool call → response）
- `cargo test --workspace` 全量通过
- 不修改任何现有实现代码（除非测试发现 bug）

## 任务分解
- [x] T1: memory crate 单元测试（InMemoryStore + SlidingWindowMemory）
  - 验证：store/get/search/clear 全路径覆盖，窗口滑动截断行为正确
- [x] T2: guardrails crate 单元测试（ContentFilter + RateLimiter + TokenBudget + ToolApproval）
  - 验证：每个 guardrail 的 pass/warn/block 三种结果路径均有测试
- [x] T3: tools crate 单元测试（ToolRegistry + ReadFile + WriteFile + Http + Shell）
  - 验证：registry CRUD、文件读写、shell 执行均有测试（Http 跳过，需外部服务）
- [x] T4: executor crate 单元测试（Runner 主循环 + ReAct/PlanAndExecute 策略）
  - 验证：正常完成、tool call 循环、handoff、max_iterations 限制均有测试
- [x] T5: 端到端集成测试（mock Provider → Runner 完整循环）
  - 验证：构建 Agent + Tool + mock Provider，跑完整 run 循环，断言最终输出和事件序列
