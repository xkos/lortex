# 测试 Checklist

> AI 生成和维护，人审核和勾选。
> 当前迭代：005a-messages-streaming

## 本迭代新增

- [ ] proto/anthropic streaming: `cargo test -p lortex-server proto::anthropic` → 11 tests passed
- [ ] messages handler: stream=true 返回 Anthropic SSE 格式
- [ ] api_formats: Model 支持多 API 格式，handler 自动选择 Provider 实现
- [ ] SSE 兼容: OpenAI provider 自动处理 SSE 格式的 non-streaming 响应
- [ ] KV store: SQLite 改为 KV+JSON 模式，17 个 store 测试通过
- [ ] 全量: `cargo test --workspace` → 293 tests passed, 0 failed

## 回归测试

- [ ] server: `cargo test -p lortex-server` → 76 tests passed
- [ ] proxy API: `cargo test -p lortex-server --test proxy_api` → 15 tests passed
- [ ] core + router + others: 216 tests passed
