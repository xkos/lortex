# 测试 Checklist

> AI 生成和维护，人审核和勾选。
> 当前迭代：005b-usage-stats

## 本迭代新增

- [x] UsageRecord 写入: 每次 LLM 调用后 usage 记录被写入
- [x] Usage 查询 API: POST /admin/api/v1/usage 返回记录列表
- [x] Usage 汇总 API: POST /admin/api/v1/usage/summary 返回汇总数据
- [x] Admin Web Usage 页面: 可查看汇总卡片 + 明细表格
- [x] 全量: `cargo test --workspace` → 265 tests passed, 0 failed

## 回归测试

- [x] server: `cargo test -p lortex-server` → 57 unit + 21 integration tests passed
- [x] proxy API: `cargo test -p lortex-server --test proxy_api` → 15 tests passed
- [x] admin API: `cargo test -p lortex-server --test admin_api` → 6 tests passed (含 usage)
- [x] core + router + others: 全部通过
