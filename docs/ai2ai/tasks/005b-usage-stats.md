# 任务 005b: 用量统计

> 状态：✅ 已关闭
> 分支：iter/005b-usage-stats
> 配对迭代：[iterations/005b-usage-stats.md](../iterations/005b-usage-stats.md)

## 迭代目标
实现用量记录写入、查询 API 和 Admin Web 统计页面。

## 验收标准（人审核/补充）
- 每次 LLM 调用写入 UsageRecord（token 分类 + credits）
- /admin/api/v1/usage 可按 key / 模型 / 时间段查询
- Admin Web 有用量统计页面
- `cargo test --workspace` 全量通过

## 任务分解
- [x] T1: UsageRecord 数据模型 + ProxyStore 扩展 + handler 写入
  - 验证：每次请求后 usage_records 有记录
- [x] T2: /admin/api/v1/usage 查询端点
  - 验证：按 key / 模型 / 时间段过滤返回正确数据
- [x] T3: Admin Web 用量统计页面
  - 验证：Web 可查看用量数据
- [x] T4: 测试验证
  - 验证：cargo test --workspace 全量通过（265 tests）
