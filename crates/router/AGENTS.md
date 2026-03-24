# router — 异构模型路由（Phase 2）

## 职责

根据任务特征、模型能力和成本预算，将 LLM 调用路由到最合适的模型。这是框架的核心差异化能力。

## 依赖

- `core` — Provider trait、Message 等类型
- `providers` — 具体的 Provider 实现

## 核心组件

- **ModelRegistry**（registry.rs）— 模型注册与能力声明
  - ModelProfile：provider、model、capabilities（各维度评分）、cost、speed、context_window、modalities
  - 注册、查询、更新模型信息
- **RoutingStrategy**（strategy.rs）— 路由策略 trait + 内置实现
  - AutoRouter：根据任务特征自动匹配模型能力
  - FixedRouter：固定使用指定模型
  - CostOptimizedRouter：在成本预算内优化质量
  - FallbackRouter：主模型 + 备选模型链
  - 混合策略：规则匹配优先 → 便宜模型分类 → 手动指定最高优先级
- **CostTracker**（cost.rs）— 成本追踪
  - 记录每次调用的 token 消耗和成本
  - 路由本身的分类调用成本单独记录
  - 按 Agent、按任务、按模型维度汇总
  - 支持成本预算告警

## 架构关键点

Router 实现了 `Provider` trait，对 Executor 来说就是一个"智能 Provider"：

```
Executor → Router(impl Provider) → 选择实际 Provider + Model → 调用 → 返回结果 + 成本
```

## 设计约束

- 异构路由是可选功能，用户可以选择不启用（直接用普通 Provider）
- 路由决策的成本必须在 metrics 中单独记录，方便对比
- 路由策略可扩展，用户可自定义 RoutingStrategy
