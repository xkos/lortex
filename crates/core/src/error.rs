//! lortex 框架的错误类型定义
//!
//! 采用分层错误体系：顶层 [`LortexError`] 包含各子模块的错误类型。

use thiserror::Error;

/// 框架顶层错误类型，包含所有子模块的错误
#[derive(Error, Debug)]
pub enum LortexError {
    #[error("Agent error: {0}")]
    Agent(#[from] AgentError),

    #[error("Tool error: {0}")]
    Tool(#[from] ToolError),

    #[error("Provider error: {0}")]
    Provider(#[from] ProviderError),

    #[error("Memory error: {0}")]
    Memory(#[from] MemoryError),

    #[error("Guardrail error: {0}")]
    Guardrail(#[from] GuardrailError),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}

/// Errors originating from Agent execution.
#[derive(Error, Debug)]
pub enum AgentError {
    #[error("Max iterations ({0}) exceeded")]
    MaxIterationsExceeded(usize),

    #[error("Handoff failed: {0}")]
    HandoffFailed(String),

    #[error("Agent not found: {0}")]
    AgentNotFound(String),

    #[error("Guardrail blocked: {0}")]
    GuardrailBlocked(String),

    #[error("{0}")]
    Other(String),
}

/// Errors originating from Tool execution.
#[derive(Error, Debug, Clone)]
pub enum ToolError {
    #[error("Tool not found: {0}")]
    NotFound(String),

    #[error("Invalid arguments: {0}")]
    InvalidArguments(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Timeout after {0}ms")]
    Timeout(u64),
}

/// Errors originating from LLM Providers.
#[derive(Error, Debug)]
pub enum ProviderError {
    #[error("API error ({status}): {message}")]
    Api { status: u16, message: String },

    #[error("Rate limited, retry after {retry_after_ms}ms")]
    RateLimited { retry_after_ms: u64 },

    #[error("Network error: {0}")]
    Network(String),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Model not supported: {0}")]
    ModelNotSupported(String),

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),
}

/// Errors originating from Memory operations.
#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Search error: {0}")]
    SearchError(String),
}

/// Errors originating from Guardrail checks.
#[derive(Error, Debug)]
pub enum GuardrailError {
    #[error("Guardrail check failed: {0}")]
    CheckFailed(String),

    #[error("Configuration error: {0}")]
    Configuration(String),
}

/// lortex 框架操作的便捷 Result 类型别名
pub type LortexResult<T> = Result<T, LortexError>;
