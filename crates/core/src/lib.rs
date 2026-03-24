//! lortex-core: Lortex 框架核心 trait 和类型定义
//!
//! 本 crate 定义了所有其他 lortex 子 crate 依赖的基础抽象：
//! - [`message`] — 统一消息格式（Role、ContentPart）
//! - [`agent`] — Agent 声明式配置（system prompt、model、tools、handoffs）
//! - [`tool`] — 工具接口（name、description、JSON Schema、execute）
//! - [`provider`] — LLM 统一接口（complete、complete_stream、embed）
//! - [`memory`] — 会话记忆接口（store、get、search）
//! - [`guardrail`] — 安全护栏接口（输入/输出校验）
//! - [`event`] — 运行期事件（用于日志、UI 更新、监控）
//! - [`error`] — 类型化错误体系

pub mod agent;
pub mod error;
pub mod event;
pub mod guardrail;
pub mod memory;
pub mod message;
pub mod provider;
pub mod tool;

pub use agent::*;
pub use error::*;
pub use event::*;
pub use guardrail::*;
pub use memory::*;
pub use message::*;
pub use provider::*;
pub use tool::*;
