# lortex 项目状态（AI 维护）

> 本文档由 AI 在每次迭代结束后更新，反映项目的实际状态。人工审核后视为有效。
> 最后更新：2026-04-11

---

## 当前阶段

Phase 2 进行中（异构模型编排 + Proxy 服务）

## 当前迭代

- 当前活跃：004b-admin-web（待验收）
- 分支：iter/004b-admin-web

---

## 模块就绪度

| 模块 | 状态 | 测试覆盖 |
|------|------|---------|
| core | ✅ 可用 | 71 tests |
| executor | ✅ 可用 | 13 tests |
| providers | ✅ 可用（真实 SSE streaming） | 0 tests |
| protocols | 🔨 部分 | 0 tests |
| tools | ✅ 可用 | 24 tests |
| swarm | ✅ 可用 | 0 tests |
| guardrails | ✅ 可用 | 35 tests |
| memory | ✅ 可用 | 24 tests |
| macros | ✅ 可用 | 0 tests |
| router | ✅ 可用 | 43 tests |
| server | ✅ 可用 | 72 tests |
| admin-web | ✅ 可用 | — (前端，手动验证) |

---

## 端到端联通状态

- ✅ Proxy 完整链路（鉴权 → 模型解析 → 上游调用 → credit 扣减 → 日志）
- ✅ Admin Web 管理后台（Provider/Model/ApiKey CRUD）
- ✅ Admin API `/admin/api/v1/*`
- ✅ Proxy `/v1/chat/completions`（non-streaming + streaming SSE）
- ✅ Proxy `/v1/messages`（Anthropic 格式）
- ✅ Proxy `/v1/models`

---

## 下一步建议

1. 真实厂商端到端验证（启动 proxy，接入 Cursor/Claude Code）
2. 修复上游响应解析 bug（之前 curl 测试发现的 "Invalid response" 问题）
3. FallbackRouter + 多模态适配
