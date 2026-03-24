# protocols — Agent 协议

## 职责

实现 Agent 间通信和工具发现的标准协议。

## 依赖

- `core` — Tool trait、Message 等类型

## MCP（Model Context Protocol）

- **types.rs** — JSON-RPC 2.0、McpToolDefinition、McpResource、McpTransport
- **server.rs** — McpServer：将本地 tools 暴露为 MCP 服务（initialize、tools/list、tools/call、resources/list）
- **client.rs** — McpClient：连接外部 MCP 服务，发现并调用远程工具；McpRemoteTool 代理调用

Transport 支持：
- SSE（已实现）
- Stdio（待完成）

## A2A（Agent-to-Agent）

- **types.rs** — AgentCard、A2ATask、A2AMessage、TaskState
- **client.rs** — A2AClient：discover、send_task、get_task、cancel_task

## 编码规范

- MCP 和 A2A 通过 feature flag 控制（`features = ["mcp", "a2a"]`）
- McpRemoteTool 实现了 core 的 Tool trait，可以无缝注入到 Agent 的工具列表中
