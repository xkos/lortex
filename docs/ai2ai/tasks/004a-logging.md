# 任务 004a: 日志 + Admin URL 规范化

> 状态：✅ 已关闭
> 分支：iter/004a-logging
> 配对迭代：[iterations/004a-logging.md](../iterations/004a-logging.md)

## 迭代目标
为 proxy 添加结构化请求日志和业务日志，规范化 Admin URL 路径。

## 验收标准（人审核/补充）
- 每个请求有 access log（method、path、status、耗时）
- 业务关键节点有日志（鉴权、模型解析、上游调用、credit 扣减）
- Admin URL 从 /admin/v1 改为 /admin/api/v1
- 所有测试更新并通过
- `cargo test --workspace` 全量通过

## 任务分解
- [x] T1: 请求日志 + 业务日志
  - 验证：启动 proxy 后请求可见结构化日志输出
- [x] T2: Admin URL 规范化（/admin/v1 → /admin/api/v1）
  - 验证：所有 admin 端点和测试更新
- [x] T3: 验证 + 测试
  - 验证：cargo test --workspace 全量通过
