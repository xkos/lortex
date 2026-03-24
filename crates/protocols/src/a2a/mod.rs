//! A2A (Agent-to-Agent) protocol support.
//!
//! Based on Google's Agent2Agent Protocol specification (v0.1.0).
//! Provides basic types and client for agent-to-agent communication.

pub mod client;
pub mod types;

pub use client::A2AClient;
pub use types::*;
