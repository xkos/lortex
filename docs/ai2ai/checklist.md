# 测试 Checklist

> AI 生成和维护，人审核和勾选。
> 当前迭代：003b-proxy-handler

## 本迭代新增

- [x] proto/openai: `cargo test -p lortex-server proto::openai` → 12 tests passed
- [x] proto/convert: `cargo test -p lortex-server proto::convert` → 9 tests passed
- [x] middleware: `cargo test -p lortex-server middleware` → 9 tests passed
- [x] proxy API: `cargo test -p lortex-server --test proxy_api` → 10 tests passed
- [x] 全量: `cargo test --workspace` → 267 tests passed, 0 failed

## 回归测试

- [x] store/sqlite: `cargo test -p lortex-server store` → 15 tests passed
- [x] admin API: `cargo test -p lortex-server --test admin_api` → 5 tests passed
- [x] core: `cargo test -p lortex-core` → 71 tests passed
- [x] executor: `cargo test -p lortex-executor` → 13 tests passed
- [x] guardrails: `cargo test -p lortex-guardrails` → 35 tests passed
- [x] memory: `cargo test -p lortex-memory` → 24 tests passed
- [x] tools: `cargo test -p lortex-tools` → 24 tests passed
- [x] router: `cargo test -p lortex-router` → 40 tests passed
- [x] e2e_runner: `cargo test --test e2e_runner` → 6 tests passed
- [x] e2e_router: `cargo test --test e2e_router` → 3 tests passed
