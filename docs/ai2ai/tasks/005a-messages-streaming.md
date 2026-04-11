# 任务 005a: Anthropic /v1/messages Streaming

> 状态：✅ 已关闭
> 分支：iter/005a-messages-streaming
> 配对迭代：[iterations/005a-messages-streaming.md](../iterations/005a-messages-streaming.md)

## 迭代目标
实现 /v1/messages 的 streaming SSE 输出（Anthropic 格式），让 Claude Code 等使用 Anthropic 协议的工具可以通过 proxy 获得流式响应。

## 验收标准（人审核/补充）
- /v1/messages stream=true 返回 Anthropic SSE 格式事件流
- 事件类型覆盖：message_start、content_block_start、content_block_delta、message_delta、message_stop
- 内部 StreamEvent 正确转换为 Anthropic SSE 格式
- credit 在 streaming 完成后异步扣减
- `cargo test --workspace` 全量通过

## 任务分解
- [x] T1: Anthropic streaming 类型定义（SSE event types）
  - 验证：序列化格式与 Anthropic API 一致
- [x] T2: /v1/messages streaming handler
  - 验证：stream=true 时返回 SSE，逐 chunk 透传
- [x] T3: 测试验证
  - 验证：集成测试 + cargo test --workspace 全量通过
- [x] T4: Model api_formats + 协议自动转换（追加）
  - 验证：根据 api_formats 自动选择 Provider 实现 + SSE 响应兼容修复
