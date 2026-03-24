# tools — 内置工具 + 注册表

## 职责

提供通用的内置工具和工具注册管理。这些是框架自带的工具，不涉及任何业务逻辑。

## 依赖

- `core` — Tool trait

## 核心组件

- **ToolRegistry**（registry.rs）— 按名称注册、查找、移除工具
- **内置工具**（builtin/）
  - ReadFileTool — 读取文件内容
  - WriteFileTool — 写入文件（requires_approval）
  - HttpTool — HTTP 请求（GET/POST/PUT/DELETE/PATCH），可配置超时
  - ShellTool — 执行 shell 命令（requires_approval），支持 working_dir 和 timeout

## 与 Taxon 的关系

Taxon 的业务工具（TaxonSearchTool、TaxonTagTool 等）不在这里实现。它们在 taxon 主 crate 中实现 core 的 Tool trait，然后注册到 ToolRegistry 中。

## 扩展性

- 第三方可以通过实现 Tool trait 开发工具插件
- 工具可以通过 MCP 协议远程加载（见 protocols/）
- `#[tool]` 宏（见 macros/）简化工具开发
