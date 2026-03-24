# executor — 执行引擎

## 职责

驱动 Agent 的运行主循环，协调 LLM 调用、工具执行、guardrails 检查和 handoff。

## 依赖

- `core` — Agent、Tool、Provider、Memory、Guardrail、Message、Event 等 trait 和类型

## 核心组件

- **Runner**（runner.rs）— 执行主循环
  - 输入 guardrails → LLM 调用 → tool 调用 → 输出 guardrails
  - 支持 handoff 递归执行
  - `run()` 阻塞执行，`run_stream()` 流式事件输出
  - RunnerConfig：max_iterations、max_tool_calls_per_turn、guardrails 开关
- **ExecutionStrategy**（strategy.rs）— 执行策略
  - ReActStrategy（默认）：观察-思考-行动循环
  - PlanAndExecuteStrategy：先规划完整计划，再逐步执行
  - 策略可扩展，用户可自定义

## 与 Router 的关系

Runner 调用 Provider.complete() 时，如果用户启用了异构路由，实际调用的是 Router（Router 实现了 Provider trait）。Runner 本身不感知路由逻辑。

## 注意事项

- Runner 是无状态的，每次 run() 调用独立
- 流式输出通过 Event channel 实现，不阻塞主循环
- max_iterations 是安全阀，防止无限循环
