# 任务 003b: Proxy 协议层 + 基础 Handler

> 状态：✅ 已关闭
> 分支：iter/003b-proxy-handler
> 配对迭代：[iterations/003b-proxy-handler.md](../iterations/003b-proxy-handler.md)

## 迭代目标
实现 OpenAI 协议类型、协议转换层、/v1/models 和 /v1/chat/completions（non-streaming）handler、API Key 鉴权中间件和 credit 扣减。

## 验收标准（人审核/补充）
- OpenAI 协议请求/响应类型完整定义
- 协议转换：OpenAI 请求 ↔ Lortex Message 双向转换正确
- /v1/models 按 API Key 返回可用模型列表
- /v1/chat/completions（non-streaming）能通过 mock provider 完成完整请求
- API Key 鉴权中间件正确拦截无效/超额 key
- 请求完成后 credit 正确扣减
- 单元测试 + 集成测试覆盖
- `cargo test --workspace` 全量通过

## 任务分解
- [x] T1: OpenAI 协议类型定义（ChatCompletionRequest/Response/StreamChunk）
  - 验证：序列化/反序列化与 OpenAI API 格式兼容
- [x] T2: 协议转换层（OpenAI ↔ Lortex Message）
  - 验证：文本消息、tool call、system prompt 转换正确
- [x] T3: API Key 鉴权中间件 + credit 扣减中间件
  - 验证：无效 key 返回 401，超额返回 429，正常请求后 credit 扣减
- [x] T4: /v1/models handler
  - 验证：按 API Key 返回模型组，格式兼容 OpenAI /v1/models 响应
- [x] T5: /v1/chat/completions handler（non-streaming）
  - 验证：mock provider 完整循环，模型寻址（PROXY_MANAGED / 精确 / 别名）
- [x] T6: 路由整合 + 集成测试
  - 验证：完整 HTTP 请求链路测试（鉴权 → 模型解析 → 转发 → credit 扣减）
