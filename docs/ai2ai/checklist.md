# 测试 Checklist

> AI 生成和维护，人审核和勾选。
> 当前迭代：006b-fallback-health-cache

## 本迭代新增

- [x] 熔断器：连续失败 3 次后 provider 标记为 Open（8 unit tests 覆盖全状态转换）
- [x] Fallback：主模型失败 → 自动尝试 fallback_models；熔断的 provider 被跳过
- [x] cache_control 透传：Anthropic ContentBlock 带 cache_control 时原样保留
- [x] Header 合并：客户端 anthropic-beta 与 model extra_headers 逗号去重合并（4 unit tests）
- [x] 全量：`cargo test --workspace` → 309 tests passed, 0 failed

## 回归测试

- [x] server: `cargo test -p lortex-server` → 72 unit + 21 integration tests passed
- [x] proxy API: `cargo test -p lortex-server --test proxy_api` → 15 tests passed
- [x] admin API: `cargo test -p lortex-server --test admin_api` → 6 tests passed
- [x] core + router + others: 全部通过
