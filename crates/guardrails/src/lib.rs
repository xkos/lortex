//! lortex-guardrails: 安全护栏
//!
//! 提供 Agent 运行时的安全和限制机制：
//! - [`ContentFilter`] — 按关键词/短语过滤危险内容
//! - [`TokenBudget`] — 限制 token 消耗总量
//! - [`ToolApproval`] — 对指定工具要求人工审批
//! - [`RateLimiter`] — 限制 LLM/工具调用频率

pub mod content_filter;
pub mod rate_limiter;
pub mod token_budget;
pub mod tool_approval;

pub use content_filter::ContentFilter;
pub use rate_limiter::RateLimiter;
pub use token_budget::TokenBudget;
pub use tool_approval::ToolApproval;
