//! 观测层 — tracing Layer 实现
//!
//! UsageLayer 通过 tracing Span 机制自动收集请求指标并写入用量记录。
//! Handler 只需 `span.record(key, value)` 打标，Layer 在 span 关闭时
//! 统一处理 credit 计算、配额扣减、UsageRecord 写库和结构化日志。

pub mod helpers;
pub mod span_data;
pub mod usage_layer;

pub use usage_layer::UsageLayer;
