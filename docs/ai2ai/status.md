# lortex 项目状态（AI 维护）

> 本文档由 AI 在每次迭代结束后更新，反映项目的实际状态。人工审核后视为有效。
> 最后更新：2026-04-10

---

## 当前阶段

Phase 1 — 基础能力（核心抽象、执行引擎、基础工具链）

## 当前迭代

- 当前活跃：001-core-tests（待验收）
- 分支：iter/001-core-tests

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
| router | ❌ 空壳 | 0 tests |

---

## 端到端联通状态

✅ mock Provider → Runner → Agent + Tool + Guardrails 完整链路已验证（6 个 e2e 测试）

---

## 已知问题 / 技术债

- Provider streaming 是伪流式（先拉完整 body 再解析）
- MCP stdio transport 未实现
- HttpTool 无测试
- providers/protocols/swarm/macros 无测试
- router crate 为空壳（Phase 2）

---

## 下一步建议

1. 补充 providers/swarm 测试（需 mock）
2. 修复 Provider 真流式 SSE 解析
3. 启动 Phase 2 — router crate 实现
