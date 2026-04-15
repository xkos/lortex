# 迭代 010: Usage Dashboard — 时间趋势 + 模型分布 + ApiKey 排行

> 分支：iter/010-usage-dashboard
> 状态：✅ 完成

## 目标
在 Admin Web 的 Usage 页面新增可视化图表：时间趋势折线图、模型消耗分布饼图、ApiKey 用量排行柱状图。

## 完成内容

### 修改文件
| 文件 | 改动 |
|------|------|
| `crates/server/src/store/traits.rs` | 新增 `TrendPoint` / `GroupedUsage` 结构体 + ProxyStore trait 新增 3 个聚合方法 |
| `crates/server/src/store/sqlite.rs` | 内存聚合实现（trend 按日分桶、by-model、by-key）+ 5 个单元测试 |
| `crates/server/src/handlers/admin/usage.rs` | 新增 `trend` / `by_model` / `by_key` 三个 handler |
| `crates/server/src/routes.rs` | 注册 `/usage/trend`、`/usage/by-model`、`/usage/by-key` 路由 |
| `crates/server/admin-web/package.json` | 新增 `echarts` + `vue-echarts` 依赖 |
| `crates/server/admin-web/src/views/Usage.vue` | ECharts 集成：趋势折线图（双轴）、模型饼图、ApiKey 柱状图 |

### 架构
```
Usage 页面
├── Summary Cards（已有）
├── Daily Trend 折线图          ← POST /usage/trend → TrendPoint[]
├── Credits by Model 饼图       ← POST /usage/by-model → GroupedUsage[]
├── Credits by API Key 柱状图   ← POST /usage/by-key → GroupedUsage[]
└── Detail Table（已有）

所有图表 + 卡片 + 表格共享同一组筛选条件（API Key + 日期范围），
点 Query 后 5 个 API 并行请求，一次刷新。
```

## 关键设计决策
- **内存聚合**：复用 `query_usage` 加载记录后 Rust 内存分组，与现有 store 模式一致，无需 SQL GROUP BY
- **ECharts 按需引入**：通过 `echarts/core` + 具体图表/组件注册，避免全量引入
- **双轴折线图**：Requests（左轴）+ Credits（右轴），量级不同适合双轴展示
- **饼图用 Credits 维度**：Credits 反映实际成本，比 requests 数更有业务价值
- **柱状图按 Credits 降序**：一目了然看到高消耗 key

## 未完成/遗留
无

## 回归影响
- 前端新增 echarts 依赖，打包体积增加约 500KB（gzip ~190KB）
- 后端新增 3 个 API 端点，不影响已有端点
- Store trait 新增 3 个方法，不影响已有方法

## 测试结果
- `cargo test --workspace`：全量通过（340+ tests, 0 failed）
- `cargo check -p lortex-server`：0 warnings
- 新增 5 个聚合单元测试
- Checklist 全部通过（11 项新增 + 7 项回归）
