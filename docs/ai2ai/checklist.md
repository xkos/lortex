# 测试 Checklist

> AI 生成和维护，人审核和勾选。
> 当前迭代：002-router-core

## 本迭代新增

- [ ] registry: `cargo test -p lortex-router registry` → 14 tests passed
- [ ] strategy: `cargo test -p lortex-router strategy` → 5 tests passed
- [ ] cost: `cargo test -p lortex-router cost` → 12 tests passed
- [ ] router: `cargo test -p lortex-router router` → 9 tests passed
- [ ] e2e: `cargo test --test e2e_router` → 3 tests passed
- [ ] 全量: `cargo test --workspace` → 216 tests passed, 0 failed

## 回归测试

- [ ] memory: `cargo test -p lortex-memory` → 24 tests passed
- [ ] guardrails: `cargo test -p lortex-guardrails` → 35 tests passed
- [ ] tools: `cargo test -p lortex-tools` → 24 tests passed
- [ ] executor: `cargo test -p lortex-executor` → 13 tests passed
- [ ] e2e_runner: `cargo test --test e2e_runner` → 6 tests passed
