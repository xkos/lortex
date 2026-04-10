//! lortex-router: 异构模型路由（Phase 2）
//!
//! 根据任务特征将请求路由到不同质量/成本的 LLM 模型。
//!
//! - [`ModelRegistry`] — 模型注册与能力声明
//! - [`RoutingStrategy`] / [`FixedRouter`] — 路由策略
//! - [`CostTracker`] — 成本追踪
//! - [`Router`] — 实现 `Provider` trait 的智能路由器

pub mod cost;
pub mod registry;
pub mod router;
pub mod strategy;

pub use registry::{
    Capabilities, CostProfile, Modality, ModelProfile, ModelRegistry,
};
pub use strategy::{
    FixedRouter, ModelSelection, RoutingError, RoutingRequest, RoutingStrategy,
};
pub use cost::{BudgetStatus, CostRecord, CostTracker};
pub use router::{Router, RouterBuilder};
