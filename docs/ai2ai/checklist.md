# 测试 Checklist

> AI 生成和维护，人审核和勾选。
> 当前迭代：010-usage-dashboard

## 本迭代新增

- [x] 趋势折线图：Usage 页面展示每日 Requests + Credits 双轴折线图
- [x] 模型饼图：Usage 页面展示各模型 Credits 消耗占比饼图
- [x] ApiKey 柱状图：Usage 页面展示各 ApiKey 的 Credits 排行柱状图
- [x] 筛选联动：选择特定 API Key 或日期范围后点 Query → 图表数据联动刷新
- [x] API — trend 端点：POST /admin/api/v1/usage/trend 返回按日分桶数据
- [x] API — by-model 端点：POST /admin/api/v1/usage/by-model 返回按模型分组数据
- [x] API — by-key 端点：POST /admin/api/v1/usage/by-key 返回按 ApiKey 分组数据
- [x] 空数据：无用量时图表空白不报错
- [x] Summary cards 不受影响：Total Requests / Tokens / Credits 卡片正常显示
- [x] Detail table 不受影响：用量明细表正常展示
- [x] 全量：`cargo test --workspace` → 340+ tests passed, 0 failed

## 回归测试

- [x] server: `cargo test -p lortex-server` → unit + integration tests passed
- [x] proxy API: `cargo test -p lortex-server --test proxy_api` → 15 tests passed
- [x] admin API: `cargo test -p lortex-server --test admin_api` → 6 tests passed
- [x] RPM/TPM 限流：正常工作（009 功能不受影响）
- [x] 熔断器 + Fallback：正常工作（006b 功能不受影响）
- [x] Credit 扣减正常：发请求后 ApiKey credit_used 增加
- [x] Usage 记录完整：Usage 表有 input_tokens/output_tokens/credits/latency_ms/ttft_ms
