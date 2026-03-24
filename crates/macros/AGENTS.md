# macros — proc-macro

## 职责

提供声明式定义工具的过程宏，简化 Tool 开发。

## 无运行时依赖

这是一个 proc-macro crate，编译时运行，不依赖 lortex 的任何运行时 crate。

## 核心宏

- **`#[tool(name = "...", description = "...")]`** — 从 async 函数生成实现 Tool trait 的 struct
  - 从函数参数推导 JSON Schema
  - 支持 String、bool、i64、f64 等类型
  - 生成 execute 实现，调用原函数并包装为 ToolOutput::text

## 使用方式

用户需要在自己的 crate 中显式依赖 macros crate 才能使用 `#[tool]` 宏。facade crate（agents）会 re-export 该宏。
