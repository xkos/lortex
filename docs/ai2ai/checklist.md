# 测试 Checklist

> AI 生成和维护，人审核和勾选。
> 当前迭代：003c-proxy-streaming

## 本迭代新增

- [x] providers streaming: OpenAI + Anthropic 真实 SSE（编译通过，运行时需真实 key 验证）
- [x] proto/anthropic: `cargo test -p lortex-server proto::anthropic` → 7 tests passed
- [x] proxy API (Anthropic): `cargo test -p lortex-server --test proxy_api messages` → 5 tests passed
- [x] 全量: `cargo test --workspace` → 288 tests passed, 0 failed

## 回归测试

- [x] server unit: `cargo test -p lortex-server --lib` → 52 tests passed
- [x] admin API: `cargo test -p lortex-server --test admin_api` → 5 tests passed
- [x] proxy API: `cargo test -p lortex-server --test proxy_api` → 15 tests passed
- [x] core: `cargo test -p lortex-core` → 71 tests passed
- [x] executor: `cargo test -p lortex-executor` → 13 tests passed
- [x] guardrails: `cargo test -p lortex-guardrails` → 35 tests passed
- [x] memory: `cargo test -p lortex-memory` → 24 tests passed
- [x] tools: `cargo test -p lortex-tools` → 24 tests passed
- [x] router: `cargo test -p lortex-router` → 40 tests passed
- [x] e2e: `cargo test --test e2e_runner --test e2e_router` → 9 tests passed
