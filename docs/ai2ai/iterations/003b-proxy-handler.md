# 迭代 003b: Proxy 协议层 + 基础 Handler

> 分支：iter/003b-proxy-handler
> 日期：2026-04-10

## 目标

实现 OpenAI 协议类型、协议转换层、proxy 鉴权中间件、/v1/models 和 /v1/chat/completions（non-streaming）handler。

## 完成内容

| 模块 | 新增测试数 | 覆盖内容 |
|------|-----------|---------|
| proto/openai | 12 | ChatCompletionRequest/Response/Chunk 序列化、消息类型（text/parts/tool_call/tool_result）、StopSequence、ErrorResponse |
| proto/convert | 9 | OpenAI ↔ Lortex Message 双向转换（user/system/assistant/tool/multipart/tool_call）、Request/Response 转换 |
| middleware/proxy_auth | 9 | API Key 提取（Bearer/x-api-key）、credit 计算（文本+缓存）、额度检查 |
| proxy_api (integration) | 10 | 鉴权拒绝（无key/错key/禁用/超额）、/v1/models 返回模型组、Anthropic auth、模型解析（not_found/not_in_group/alias/PROXY_MANAGED） |

新增 40 个测试，server crate 共 60 个测试。Workspace 共 267 tests 全部通过。

## 架构验证

- proxy 鉴权 → 模型解析 → provider 构建 → LLM 调用 → credit 扣减 完整链路已连通
- PROXY_MANAGED / 精确 ID / 别名 三种模型寻址方式均可用
- OpenAI 和 Anthropic 两种 auth 格式均支持

## 未完成 / 遗留

- 003c：streaming SSE、Anthropic /v1/messages 入口、providers 真实 SSE 修复

## 回归影响

无。仅在 server crate 内新增文件，未修改任何现有 crate。
