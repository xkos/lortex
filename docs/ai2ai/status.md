# lortex 项目状态（AI 维护）

> 本文档由 AI 在每次迭代结束后更新，反映项目的实际状态。人工审核后视为有效。
> 最后更新：2026-04-11

---

## 当前阶段

Phase 2 进行中（异构模型编排 + Proxy 服务）

## 当前迭代

- 当前活跃：006a-mvp-polish（已完成，待验收合并）
- 分支：iter/006a-mvp-polish

---

## 模块就绪度

| 模块 | 状态 | 测试覆盖 |
|------|------|---------|
| core | ✅ 可用 | 71 tests |
| executor | ✅ 可用 | 13 tests |
| providers | ✅ 可用（extra_headers + cache token） | 0 tests |
| protocols | 🔨 部分 | 0 tests |
| tools | ✅ 可用 | 24 tests |
| swarm | ✅ 可用 | 0 tests |
| guardrails | ✅ 可用 | 35 tests |
| memory | ✅ 可用 | 24 tests |
| macros | ✅ 可用 | 0 tests |
| router | ✅ 可用 | 43 tests |
| server | ✅ 可用 | 78 tests |
| admin-web | ✅ 可用 | — (前端) |

---

## Proxy 功能完成度

| 端点 | Non-streaming | Streaming |
|------|:---:|:---:|
| `/v1/chat/completions` (OpenAI) | ✅ | ✅ |
| `/v1/messages` (Anthropic) | ✅ | ✅ |
| `/v1/models` | ✅ | — |
| Admin API `/admin/api/v1/*` | ✅ | — |
| Admin Web `/admin/web/*` | ✅ | — |

| 功能 | 状态 |
|------|:---:|
| API Key 鉴权 | ✅ |
| Credit 扣减 | ✅ |
| 模型寻址（PROXY_MANAGED/ID/别名） | ✅ |
| api_formats 协议自动选择 | ✅ |
| SSE 响应兼容（中转服务） | ✅ |
| 结构化日志 | ✅ |
| Admin Web 管理后台 | ✅ |
| 用量统计（记录+查询+Web） | ✅ |
| Model update endpoint | ✅ |
| extra_headers 注入 | ✅ |
| handler 去重（shared 模块） | ✅ |
| cache token 传递 | ✅ |

---

## 下一步建议

1. 006b: FallbackRouter + 健康检测
2. /v1/embeddings 实现
3. Rate Limiting（RPM/TPM per ApiKey）
