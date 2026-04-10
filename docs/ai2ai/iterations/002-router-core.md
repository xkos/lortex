# 迭代 002: Router 核心骨架

> 分支：iter/002-router-core
> 日期：2026-04-10

## 目标

实现 Router crate 的核心骨架：模型注册、固定路由策略、成本追踪，并让 Router 实现 Provider trait 对 Executor 透明。

## 完成内容

| 模块 | 新增测试数 | 覆盖内容 |
|------|-----------|---------|
| registry | 14 | ModelProfile、Capabilities、CostProfile、ModelRegistry（注册/查询/过滤） |
| strategy | 5 | RoutingStrategy trait、FixedRouter、RoutingRequest、ModelSelection |
| cost | 12 | CostTracker（记录/查询/预算告警/重置）、BudgetStatus |
| router | 9 | Router 实现 Provider trait、RouterBuilder、路由分发、成本记录、能力聚合 |
| e2e (tests/) | 3 | Router → Runner 完整循环：text response、tool call + cost tracking、routing error |

Router crate 从空壳变为 43 个测试覆盖的完整实现。Workspace 共 216 tests 全部通过。

## 架构验证

核心设计决策得到验证：
- Router 实现 Provider trait，对 Executor 完全透明 — Runner 不需要任何改动
- RoutingStrategy 是可扩展的 trait，FixedRouter 是最简实现
- CostTracker 在每次 LLM 调用后自动记录，支持预算告警

## 未完成 / 遗留

- AutoRouter（按任务特征自动匹配）— 下一迭代
- CostOptimizedRouter（在预算内优化质量）— 下一迭代
- FallbackRouter（主模型 + 备选模型链）— 下一迭代
- Router streaming 实现（当前返回空 stream）

## 回归影响

无。仅新增 router crate 代码和 facade re-export，未修改任何现有 crate 的实现代码。
