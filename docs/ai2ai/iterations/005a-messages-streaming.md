# 迭代 005a: Anthropic /v1/messages Streaming + api_formats

> 分支：iter/005a-messages-streaming
> 日期：2026-04-11

## 目标

实现 /v1/messages streaming SSE 输出，新增 Model api_formats 字段实现协议自动选择，修复 SSE 响应兼容问题。

## 完成内容

| 模块 | 说明 |
|------|------|
| proto/anthropic | Streaming SSE 类型（MessageStart/ContentBlockStart/Delta/Stop/MessageDelta/MessageStop）+ 4 个测试 |
| handlers/messages | 完整 streaming 支持：stream=true 返回 Anthropic SSE 格式事件流 |
| models/model | 新增 `ApiFormat` 枚举 + `api_formats: Vec<ApiFormat>` 字段 |
| handlers/provider_builder | 新增共享 Provider 构建逻辑，根据 api_formats 和请求格式自动选择 |
| handlers/chat + messages | 重构使用 provider_builder |
| providers/openai | SSE 响应兼容：non-streaming 请求收到 SSE 格式时自动重组为标准 JSON |
| store/sqlite | **重构为 KV+JSON 模式**：单表 entities，核心数据 JSON 存储，结构体变更无需 migration |
| admin-web/Models | 新增 API Formats 复选框 + 表格显示 |

Workspace 共 293 tests 全部通过。

## 关键设计

- `api_formats` 是数组，一个模型可以支持多种格式
- handler 根据请求入口选择 preferred format，如果模型不支持则 fallback 到模型支持的第一个格式
- OpenAI provider 的 `complete` 方法增加 SSE 响应自动重组，兼容返回 SSE 的中转服务

## 回归影响

- SQLite 存储完全重构为 KV+JSON 模式，旧 db 不兼容（需删除重建，但这是最后一次）
- 后续结构体加字段只需 `#[serde(default)]`，永远不需要 migration
