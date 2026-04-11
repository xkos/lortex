# 测试 Checklist

> AI 生成和维护，人审核和勾选。
> 当前迭代：004a-logging

## 本迭代新增

- [ ] Admin URL: 所有 `/admin/api/v1/*` 端点正常工作
- [ ] 日志: 启动 proxy 后请求可见结构化日志
- [ ] 全量: `cargo test --workspace` → 288 tests passed, 0 failed

## 回归测试

- [ ] server: `cargo test -p lortex-server` → 72 tests passed
- [ ] core: `cargo test -p lortex-core` → 71 tests passed
- [ ] router: `cargo test -p lortex-router` → 40 tests passed
- [ ] executor + guardrails + memory + tools: 107 tests passed
- [ ] e2e: `cargo test --test e2e_runner --test e2e_router` → 9 tests passed
