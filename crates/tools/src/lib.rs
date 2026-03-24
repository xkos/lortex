//! lortex-tools: 内置工具集与工具注册表
//!
//! 提供框架自带的通用工具和工具管理：
//! - [`ToolRegistry`] — 按名称注册、查找、移除工具的动态注册表
//! - 内置工具：[`ReadFileTool`]、[`WriteFileTool`]、[`ShellTool`]、[`HttpTool`]

pub mod builtin;
pub mod registry;

pub use builtin::file::{ReadFileTool, WriteFileTool};
pub use builtin::http::HttpTool;
pub use builtin::shell::ShellTool;
pub use registry::ToolRegistry;
