# 测试 Checklist

> AI 生成和维护，人审核和勾选。
> 当前迭代：004b-admin-web

## 本迭代新增

- [x] 后端: `cargo build -p lortex-server` 编译通过（含 rust-embed）
- [x] CLI: `--with-admin-web` 参数可见
- [x] 前端: `npm run build` 构建成功
- [x] 全量: `cargo test --workspace` → 288 tests passed, 0 failed
- [x] 手动: 启动 proxy --with-admin-web，访问 /admin/web/ 可见登录页

## 回归测试

- [x] server: `cargo test -p lortex-server` → 72 tests passed
- [x] core + executor + guardrails + memory + tools: 167 tests passed
- [x] router: `cargo test -p lortex-router` → 40 tests passed
- [x] e2e: 9 tests passed
