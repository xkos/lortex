# 测试 Checklist

> AI 生成和维护，人审核和勾选。
> 当前迭代：001-core-tests

## 本迭代新增

- [x] memory: `cargo test -p lortex-memory` → 24 tests passed
- [x] guardrails: `cargo test -p lortex-guardrails` → 35 tests passed
- [x] tools: `cargo test -p lortex-tools` → 24 tests passed
- [x] executor: `cargo test -p lortex-executor` → 13 tests passed
- [x] e2e: `cargo test --test e2e_runner` → 6 tests passed
- [x] 全量: `cargo test --workspace` → 173 tests passed, 0 failed

## 回归测试

（首次迭代无回归项）
