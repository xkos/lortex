# 任务 002: Router 核心骨架

> 状态：✅ 已关闭
> 分支：iter/002-router-core
> 配对迭代：[iterations/002-router-core.md](../iterations/002-router-core.md)

## 迭代目标
实现 Router crate 的核心骨架：模型注册、固定路由策略、成本追踪，并让 Router 实现 Provider trait 对 Executor 透明。

## 验收标准（人审核/补充）
- ModelProfile 和 ModelRegistry 可注册模型并查询能力
- RoutingStrategy trait 定义清晰，FixedRouter 可按指定模型路由
- CostTracker 能记录和查询 token 消耗与成本
- Router 实现 Provider trait，可作为 Runner 的 provider 使用
- 单元测试覆盖所有新增公开 API
- `cargo test --workspace` 全量通过
- 不修改 executor/providers 等现有 crate 的代码

## 任务分解
- [x] T1: ModelProfile + ModelRegistry（模型注册与能力声明）
  - 验证：注册/查询/列举模型，按能力过滤
- [x] T2: RoutingStrategy trait + FixedRouter
  - 验证：FixedRouter 始终返回指定模型，trait 可扩展
- [x] T3: CostTracker（成本追踪）
  - 验证：记录调用成本，按模型/总量查询，预算告警
- [x] T4: Router 实现 Provider trait
  - 验证：Router 作为 Provider 传入 Runner，完整 run 循环通过
- [x] T5: 集成测试（Router + mock Provider → Runner）
  - 验证：Router 选择正确的 Provider，成本被记录，事件正常
