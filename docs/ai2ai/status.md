# lortex 项目状态（AI 维护）

> 本文档由 AI 在每次迭代结束后更新，反映项目的实际状态。人工审核后视为有效。
> 最后更新：2026-04-10

---

## 当前阶段

Phase 2 进行中（异构模型编排 + Proxy 服务）

## 当前迭代

- 当前活跃：003b-proxy-handler（待验收）
- 分支：iter/003b-proxy-handler

---

## 模块就绪度

| 模块 | 状态 | 测试覆盖 |
|------|------|---------|
| core | ✅ 可用 | 71 tests |
| executor | ✅ 可用 | 13 tests |
| providers | ✅ 可用 | 0 tests |
| protocols | 🔨 部分 | 0 tests |
| tools | ✅ 可用 | 24 tests |
| swarm | ✅ 可用 | 0 tests |
| guardrails | ✅ 可用 | 35 tests |
| memory | ✅ 可用 | 24 tests |
| macros | ✅ 可用 | 0 tests |
| router | ✅ 可用 | 43 tests |
| server | 🔨 部分 | 60 tests（存储 + Admin + 协议 + 鉴权 + proxy handler，streaming 待 003c） |

---

## 端到端联通状态

- ✅ mock Provider → Runner → Agent + Tool + Guardrails（6 tests）
- ✅ Router → Runner → Agent + Tool + CostTracker（3 tests）
- ✅ Admin API → SQLite Store → CRUD（5 tests）
- ✅ Proxy API → 鉴权 → 模型解析 → Provider 构建（10 tests）
- ⏳ Streaming SSE + Anthropic 入口（003c）

---

## 已知问题 / 技术债

- Provider streaming 是伪流式（先拉完整 body 再解析）
- Router streaming 未实现（返回空 stream）
- MCP stdio transport 未实现
- HttpTool 无测试
- providers/protocols/swarm/macros 无测试

---

## 下一步建议

1. 003c：Streaming SSE + Anthropic /v1/messages 入口 + providers 真实 SSE 修复
2. 004：FallbackRouter + 多模态适配
