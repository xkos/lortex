# lortex 项目状态（AI 维护）

> 本文档由 AI 在每次迭代结束后更新，反映项目的实际状态。人工审核后视为有效。
> 最后更新：2026-04-10

---

## 当前阶段

Phase 1 完成，Phase 2 进行中（异构模型编排）

## 当前迭代

- 当前活跃：002-router-core（待验收）
- 分支：iter/002-router-core

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

---

## 端到端联通状态

- ✅ mock Provider → Runner → Agent + Tool + Guardrails（6 tests）
- ✅ Router → Runner → Agent + Tool + CostTracker（3 tests）

---

## 已知问题 / 技术债

- Provider streaming 是伪流式（先拉完整 body 再解析）
- Router streaming 未实现（返回空 stream）
- MCP stdio transport 未实现
- HttpTool 无测试
- providers/protocols/swarm/macros 无测试

---

## 下一步建议

1. 实现 FallbackRouter（主模型 + 备选模型链）
2. 实现 AutoRouter（按任务特征自动匹配）
3. 修复 Provider 真流式 SSE 解析
4. 补充 providers/swarm 测试
