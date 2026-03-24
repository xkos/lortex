# swarm — 多 Agent 编排

## 职责

协调多个 Agent 协作完成复杂任务。

## 依赖

- `core` — Agent、RunInput/RunOutput 等类型
- `executor` — Runner（执行单个 Agent）

## 核心组件

- **Orchestrator**（orchestrator.rs）— 编排器，根据 pattern 协调多个 Agent
  - OrchestratorBuilder：配置 pattern 和 runner
- **OrchestrationPattern**（patterns.rs）— 编排模式
  - Router：分诊 Agent 根据任务类型路由到专家 Agent
  - Pipeline：按阶段顺序执行，前一阶段输出作为下一阶段输入
  - Parallel：多 Agent 并行执行，由 aggregator 汇总结果
  - Hierarchical：supervisor 协调 workers（通过 handoff 实现）

## 注意事项

- 当前不做通用工作流引擎，复杂流程由业务层用普通代码编排
- Phase 4 可能增加轻量 DAG 引擎（条件分支 + 并行，不支持循环）
