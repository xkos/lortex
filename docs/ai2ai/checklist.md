# 测试 Checklist

> AI 生成和维护，人审核和勾选。
> 当前迭代：007-estimated-chars

## 本迭代新增

- [x] estimated_chars 记录：发一个简单请求（如 "Say hello"），检查 Usage 表中 Est.Chars 列有合理值（几十到几百）
- [x] 异常检测对比：Est.Chars 值与 Input tokens 对比 — 如果 Input tokens 远大于 Est.Chars / 3，说明上游有额外注入
- [ x 旧数据兼容：历史 Usage 记录的 Est.Chars 列显示 `-`
- [x] Usage 页面：Est.Chars 列位于 Cache R 和 Credits 之间
- [x] 全量：`cargo test --workspace` → 324 tests passed, 0 failed

## 回归测试

- [x] server: `cargo test -p lortex-server` → unit + integration tests passed
- [x] proxy API: `cargo test -p lortex-server --test proxy_api` → 15 tests passed
- [x] admin API: `cargo test -p lortex-server --test admin_api` → 6 tests passed
- [x] 熔断器 + Fallback：正常工作（006b 功能不受影响）
- [x] cache_control 透传：正常工作（006b 功能不受影响）
