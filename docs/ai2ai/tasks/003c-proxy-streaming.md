# 任务 003c: Streaming SSE + Anthropic 入口

> 状态：🔨 进行中
> 分支：iter/003c-proxy-streaming
> 配对迭代：[iterations/003c-proxy-streaming.md](../iterations/003c-proxy-streaming.md)

## 迭代目标
修复 providers 真实 SSE streaming，实现 proxy 的 streaming 透传和 Anthropic /v1/messages 入口。

## 验收标准（人审核/补充）
- OpenAI provider 的 complete_stream 逐 chunk 产出 StreamEvent（不再等完整 body）
- Anthropic provider 同上
- /v1/chat/completions 支持 stream=true，返回 SSE 格式
- /v1/messages Anthropic 格式入口可用
- 单元测试 + 集成测试覆盖
- `cargo test --workspace` 全量通过

## 任务分解
- [ ] T1: OpenAI provider 真实 SSE streaming 修复
  - 验证：complete_stream 逐 chunk 产出 ContentDelta 事件
- [ ] T2: Anthropic provider 真实 SSE streaming 修复
  - 验证：同上
- [ ] T3: /v1/chat/completions streaming handler
  - 验证：stream=true 时返回 SSE 格式，逐 chunk 透传
- [ ] T4: Anthropic 协议类型 + /v1/messages handler
  - 验证：Anthropic 格式请求可路由到后端
- [ ] T5: 集成测试
  - 验证：streaming + Anthropic 入口端到端测试
