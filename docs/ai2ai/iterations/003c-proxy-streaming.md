# 迭代 003c: Streaming SSE + Anthropic 入口

> 分支：iter/003c-proxy-streaming
> 日期：2026-04-10

## 目标

修复 providers 真实 SSE streaming，实现 proxy 的 streaming 透传和 Anthropic /v1/messages 入口。

## 完成内容

| 模块 | 说明 |
|------|------|
| providers/openai | 真实增量 SSE streaming（替换伪流式），支持 ContentDelta + ToolCall + Done + Usage |
| providers/anthropic | 同上，支持 Anthropic SSE 事件格式（message_start/content_block_delta/message_delta） |
| server/handlers/chat | streaming 路径：stream=true 时返回 SSE，逐 chunk 透传，credit 异步扣减 |
| server/proto/anthropic | Anthropic Messages API 协议类型（Request/Response/ContentBlock/Error）+ 7 个单元测试 |
| server/proto/convert | Anthropic ↔ Lortex Message 双向转换 |
| server/handlers/messages | /v1/messages handler（non-streaming，streaming 标记为 TODO） |
| proxy_api tests | 新增 5 个 Anthropic 端点测试 |

新增 Anthropic 协议类型 7 个测试 + proxy API 5 个测试。Workspace 共 288 tests 全部通过。

## 框架层修复

providers crate 的 streaming 从伪流式（`resp.text().await` 等完整 body）改为真实增量 SSE（`resp.bytes_stream()` 逐 chunk 解析）。这是 proxy 产品反向推动框架优化的第一个实例。

## 未完成 / 遗留

- /v1/messages streaming（Anthropic SSE 格式输出）— 标记为 TODO
- 真实厂商端到端测试（需要有效 API key）

## 回归影响

providers crate 的 `complete_stream` 方法签名不变，但内部实现完全重写。新增 `async-stream` 依赖。现有使用 `complete_stream` 的代码（executor 的 `run_stream`）不受影响。
