# lortex 项目状态（AI 维护）

> 本文档由 AI 在每次迭代结束后更新，反映项目的实际状态。人工审核后视为有效。
> 最后更新：2026-04-10

---

## 当前阶段

Phase 2 进行中（异构模型编排 + Proxy 服务）

## 当前迭代

- 当前活跃：003c-proxy-streaming（待验收）
- 分支：iter/003c-proxy-streaming

---

## 模块就绪度

| 模块 | 状态 | 测试覆盖 |
|------|------|---------|
| core | ✅ 可用 | 71 tests |
| executor | ✅ 可用 | 13 tests |
| providers | ✅ 可用（streaming 已修复） | 0 tests |
| protocols | 🔨 部分 | 0 tests |
| tools | ✅ 可用 | 24 tests |
| swarm | ✅ 可用 | 0 tests |
| guardrails | ✅ 可用 | 35 tests |
| memory | ✅ 可用 | 24 tests |
| macros | ✅ 可用 | 0 tests |
| router | ✅ 可用 | 43 tests |
| server | ✅ 可用 | 72 tests（存储 + Admin + 协议 + 鉴权 + proxy handler + Anthropic） |

---

## 端到端联通状态

- ✅ mock Provider → Runner → Agent + Tool + Guardrails（6 tests）
- ✅ Router → Runner → Agent + Tool + CostTracker（3 tests）
- ✅ Admin API → SQLite Store → CRUD（5 tests）
- ✅ Proxy /v1/chat/completions（non-streaming + streaming SSE）
- ✅ Proxy /v1/messages（Anthropic 格式入口）
- ✅ Proxy /v1/models（按 API Key 返回模型组）
- ✅ API Key 鉴权（Bearer + x-api-key）+ credit 扣减

---

## 已知问题 / 技术债

- /v1/messages streaming 未实现（non-streaming 可用）
- MCP stdio transport 未实现
- HttpTool 无测试
- providers/protocols/swarm/macros 无单元测试

---

## 下一步建议

1. 真实厂商端到端测试（用真实 API key 验证完整链路）
2. 004：FallbackRouter + 多模态适配
3. /v1/messages streaming 实现
