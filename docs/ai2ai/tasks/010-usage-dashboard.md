# 任务 010: Usage Dashboard — 时间趋势 + 模型分布 + ApiKey 排行

> 状态：✅ 已关闭
> 分支：iter/010-usage-dashboard
> 配对迭代：[iterations/010-usage-dashboard.md](../iterations/010-usage-dashboard.md)

## 迭代目标
在 Admin Web 的 Usage 页面新增可视化图表：时间趋势折线图、模型消耗分布饼图、ApiKey 用量排行柱状图。

## 设计摘要

### 后端
- ProxyStore trait 新增 3 个聚合方法：`usage_trend`、`usage_by_model`、`usage_by_key`
- 返回类型：`TrendPoint`（bucket + 聚合值）、`GroupedUsage`（group_key + 聚合值）
- SQLite 实现：复用 query_usage 加载记录后在 Rust 内存中分组聚合（与现有模式一致）
- Admin API 新增 3 个 POST 端点

### 前端
- 引入 ECharts（`echarts` + `vue-echarts`）
- Usage.vue 上方保留 summary cards，下方新增 3 个图表区域
- 图表跟随筛选条件联动刷新

### 不做
- 多租户隔离（ApiKey 级别隔离已足够）
- 实时 WebSocket 推送
- SQL 级别 GROUP BY 优化（当前数据量不需要）

## 验收标准（人审核/补充）
- 时间趋势图展示每日 requests / tokens / credits 曲线
- 模型分布图展示各模型 credits 消耗占比
- ApiKey 排行图展示各 key 的 credits / requests 排名
- 图表跟随 API Key 筛选和日期范围联动
- 已有 Usage 页面功能不受影响（summary cards + detail table）
- `cargo test --workspace` 全量通过

## 任务分解
- [x] T1: Store trait + 聚合数据结构
  - 新增 `TrendPoint` / `GroupedUsage` 结构体
  - ProxyStore trait 新增 `usage_trend` / `usage_by_model` / `usage_by_key`
  - 验证：编译通过，结构体可 Serialize
- [x] T2: SQLite 实现 — 内存聚合
  - `usage_trend`：按 day 分桶，统计 requests / input_tokens / output_tokens / credits
  - `usage_by_model`：按 provider_id/vendor_model_name 分组
  - `usage_by_key`：按 api_key_id + api_key_name 分组
  - 验证：5 个单元测试覆盖空数据、多桶、多组、时间筛选
- [x] T3: Admin API 新增 3 个聚合端点
  - POST `/usage/trend` → `Vec<TrendPoint>`
  - POST `/usage/by-model` → `Vec<GroupedUsage>`
  - POST `/usage/by-key` → `Vec<GroupedUsage>`
  - 验证：路由注册成功，handler 逻辑正确
- [x] T4: 前端 — ECharts 集成 + 图表组件
  - 安装 echarts + vue-echarts
  - Usage.vue 新增 3 个图表：趋势折线图、模型饼图、ApiKey 柱状图
  - 图表与现有 filter 联动
  - 验证：npm run build 成功
- [x] T5: 全量测试 + 归档
  - `cargo test --workspace` 340 tests passed, 0 failed
  - checklist 生成
