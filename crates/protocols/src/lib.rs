//! lortex-protocols: Agent 协议实现
//!
//! 提供 Agent 间通信和工具发现的标准协议：
//! - [`mcp`] — MCP（Model Context Protocol）客户端和服务端
//! - [`a2a`] — A2A（Agent-to-Agent）协议支持

#[cfg(feature = "mcp")]
pub mod mcp;

#[cfg(feature = "a2a")]
pub mod a2a;
