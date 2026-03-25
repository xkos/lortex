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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_error_display() {
        assert_eq!(
            AgentError::MaxIterationsExceeded(10).to_string(),
            "Max iterations (10) exceeded"
        );
        assert_eq!(
            AgentError::HandoffFailed("timeout".into()).to_string(),
            "Handoff failed: timeout"
        );
        assert_eq!(
            AgentError::AgentNotFound("x".into()).to_string(),
            "Agent not found: x"
        );
        assert_eq!(
            AgentError::GuardrailBlocked("toxic".into()).to_string(),
            "Guardrail blocked: toxic"
        );
        assert_eq!(
            AgentError::Other("misc".into()).to_string(),
            "misc"
        );
    }

    #[test]
    fn tool_error_display() {
        assert_eq!(
            ToolError::NotFound("search".into()).to_string(),
            "Tool not found: search"
        );
        assert_eq!(
            ToolError::InvalidArguments("bad json".into()).to_string(),
            "Invalid arguments: bad json"
        );
        assert_eq!(
            ToolError::ExecutionFailed("crash".into()).to_string(),
            "Execution failed: crash"
        );
        assert_eq!(
            ToolError::PermissionDenied("no auth".into()).to_string(),
            "Permission denied: no auth"
        );
        assert_eq!(ToolError::Timeout(5000).to_string(), "Timeout after 5000ms");
    }

    #[test]
    fn provider_error_display() {
        assert_eq!(
            ProviderError::Api {
                status: 429,
                message: "rate limit".into()
            }
            .to_string(),
            "API error (429): rate limit"
        );
        assert_eq!(
            ProviderError::RateLimited {
                retry_after_ms: 1000
            }
            .to_string(),
            "Rate limited, retry after 1000ms"
        );
        assert_eq!(
            ProviderError::Network("dns fail".into()).to_string(),
            "Network error: dns fail"
        );
        assert_eq!(
            ProviderError::ModelNotSupported("gpt-5".into()).to_string(),
            "Model not supported: gpt-5"
        );
    }

    #[test]
    fn memory_error_display() {
        assert_eq!(
            MemoryError::SessionNotFound("abc".into()).to_string(),
            "Session not found: abc"
        );
        assert_eq!(
            MemoryError::StorageError("disk full".into()).to_string(),
            "Storage error: disk full"
        );
        assert_eq!(
            MemoryError::SearchError("index broken".into()).to_string(),
            "Search error: index broken"
        );
    }

    #[test]
    fn guardrail_error_display() {
        assert_eq!(
            GuardrailError::CheckFailed("bad input".into()).to_string(),
            "Guardrail check failed: bad input"
        );
        assert_eq!(
            GuardrailError::Configuration("missing key".into()).to_string(),
            "Configuration error: missing key"
        );
    }

    #[test]
    fn lortex_error_from_agent_error() {
        let err: LortexError = AgentError::MaxIterationsExceeded(5).into();
        assert!(matches!(err, LortexError::Agent(_)));
        assert!(err.to_string().contains("5"));
    }

    #[test]
    fn lortex_error_from_tool_error() {
        let err: LortexError = ToolError::NotFound("x".into()).into();
        assert!(matches!(err, LortexError::Tool(_)));
    }

    #[test]
    fn lortex_error_from_provider_error() {
        let err: LortexError = ProviderError::Network("timeout".into()).into();
        assert!(matches!(err, LortexError::Provider(_)));
    }

    #[test]
    fn lortex_error_from_memory_error() {
        let err: LortexError = MemoryError::SessionNotFound("s1".into()).into();
        assert!(matches!(err, LortexError::Memory(_)));
    }

    #[test]
    fn lortex_error_from_guardrail_error() {
        let err: LortexError = GuardrailError::CheckFailed("blocked".into()).into();
        assert!(matches!(err, LortexError::Guardrail(_)));
    }

    #[test]
    fn lortex_error_from_serde_json() {
        let bad_json = serde_json::from_str::<serde_json::Value>("not json");
        let serde_err = bad_json.unwrap_err();
        let err: LortexError = serde_err.into();
        assert!(matches!(err, LortexError::Serialization(_)));
    }

    #[test]
    fn lortex_error_other() {
        let err = LortexError::Other("unexpected".into());
        assert_eq!(err.to_string(), "unexpected");
    }
}
