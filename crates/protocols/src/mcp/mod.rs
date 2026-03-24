//! MCP (Model Context Protocol) implementation.
//!
//! Provides both client (connect to MCP servers) and server (expose tools as MCP).

pub mod client;
pub mod server;
pub mod types;

pub use client::McpClient;
pub use server::McpServer;
pub use types::*;
