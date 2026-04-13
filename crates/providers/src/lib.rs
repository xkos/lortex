//! lortex-providers: LLM 提供商实现
//!
//! 为主流 LLM 服务提供 `Provider` trait 的具体实现：
//! - [`openai`] — OpenAI GPT 系列
//! - [`anthropic`] — Anthropic Claude 系列

use serde::{Deserialize, Serialize};

#[cfg(feature = "openai")]
pub mod openai;

#[cfg(feature = "anthropic")]
pub mod anthropic;

/// 自动 prompt cache 注入策略
///
/// 控制 proxy 在转发请求时自动注入 `cache_control: {"type": "ephemeral"}` 的位置。
/// Anthropic 模型最多支持 4 个 cache breakpoint；非 Anthropic 模型会忽略额外字段。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheStrategy {
    /// 不注入，由客户端自行控制
    None,
    /// 仅缓存 system prompt（1 个 breakpoint）
    SystemOnly,
    /// 缓存 system prompt + tools（2 个 breakpoint）
    Standard,
    /// 缓存 system + tools + 倒数第二条 user 消息（3 个 breakpoint）
    /// 参考 Claude Code 的缓存策略
    Full,
}

impl Default for CacheStrategy {
    fn default() -> Self {
        Self::Full
    }
}

impl CacheStrategy {
    pub fn from_str(s: &str) -> Self {
        match s {
            "none" => Self::None,
            "system_only" => Self::SystemOnly,
            "standard" => Self::Standard,
            "full" => Self::Full,
            _ => Self::Full,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::None => "none",
            Self::SystemOnly => "system_only",
            Self::Standard => "standard",
            Self::Full => "full",
        }
    }
}
