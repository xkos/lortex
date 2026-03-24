//! lortex-providers: LLM 提供商实现
//!
//! 为主流 LLM 服务提供 `Provider` trait 的具体实现：
//! - [`openai`] — OpenAI GPT 系列
//! - [`anthropic`] — Anthropic Claude 系列

#[cfg(feature = "openai")]
pub mod openai;

#[cfg(feature = "anthropic")]
pub mod anthropic;
