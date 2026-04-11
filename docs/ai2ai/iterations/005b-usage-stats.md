# 迭代 005b: 用量统计

> 分支：iter/005b-usage-stats
> 状态：✅ 已完成
> 日期：2026-04-11

## 目标
实现用量记录写入、查询 API 和 Admin Web 统计页面。

## 完成内容

### T1: UsageRecord 存储 + 写入
- 新增 `UsageRecord` 数据模型（token 分类 + credits + latency）
- `ProxyStore` trait 扩展：`insert_usage`、`query_usage`、`summarize_usage`
- SQLite KV+JSON 模式实现 usage 存储（kind='usage'）
- `deduct_credits` 扩展 endpoint/latency_ms 参数，每次调用写入 UsageRecord
- 更新所有 4 个调用点（chat/messages × blocking/streaming）

### T2: Usage 查询 API
- `POST /admin/api/v1/usage` — 按条件查询用量记录
- `POST /admin/api/v1/usage/summary` — 用量汇总统计
- 支持按 api_key_id / provider_id / vendor_model_name / 时间范围筛选

### T3: Admin Web 用量页面
- 汇总卡片：请求数、Input Tokens、Output Tokens、Credits
- 明细表格：时间、API Key、模型、端点、token、credits、延迟
- 筛选器：API Key 下拉 + 时间范围选择

### T4: 测试验证
- `cargo test --workspace` → 265 tests passed, 0 failed

## 未完成 / 遗留
- Streaming 路径 latency_ms 始终为 0（无法准确测量流式请求耗时）
- Cache token 从上游响应提取尚未实现（当前传 0）

## 回归影响
- `deduct_credits` 签名变更（新增 endpoint + latency_ms），已更新全部调用点
- 无破坏性变更

## 测试结果
继承自 005a checklist（全部通过）+ 新增 usage_query_and_summary 集成测试
