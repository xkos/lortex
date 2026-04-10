# 迭代 001: 核心 crate 单元测试 + 端到端集成测试

> 分支：iter/001-core-tests
> 日期：2026-04-10

## 目标

为 executor、memory、guardrails、tools 四个 crate 补充单元测试，并新增端到端集成测试验证 Runner 完整循环。

## 完成内容

| Crate | 新增测试数 | 覆盖内容 |
|-------|-----------|---------|
| memory | 24 | InMemoryStore（12）+ SlidingWindowMemory（12）：store/get/search/clear 全路径 |
| guardrails | 35 | ContentFilter（11）+ RateLimiter（6）+ TokenBudget（10）+ ToolApproval（8） |
| tools | 24 | ToolRegistry（9）+ ReadFileTool（4）+ WriteFileTool（4）+ ShellTool（7） |
| executor | 13 | Runner 主循环：text response、tool call、unknown tool、handoff、max_iterations、guardrails block/disable |
| e2e (tests/) | 6 | 完整链路：tool call cycle、input/output guardrail block、handoff、multi-step、max iterations |

总计新增 102 个测试，加上 core 已有的 71 个，workspace 共 173 个测试全部通过。

## 未完成 / 遗留

- HttpTool 未写测试（需要 HTTP mock server，超出本迭代范围）
- providers crate（OpenAI/Anthropic）未写测试（涉及外部 API mock，计划下一迭代）
- protocols crate 未写测试（MCP stdio 本身未实现）
- swarm crate 未写测试（依赖 executor，可在下一迭代补充）

## 回归影响

无。本迭代仅新增测试文件和 dev-dependency（tempfile），未修改任何实现代码。

## 测试结果

见 [checklist.md](../checklist.md)
