//! lortex-swarm: 多 Agent 编排
//!
//! 提供协调多个 Agent 协作的编排模式：
//! - Router — 分诊 Agent 根据任务类型路由到专家 Agent
//! - Pipeline — 按阶段顺序执行，前一阶段输出作为下一阶段输入
//! - Parallel — 多 Agent 并行执行，由 aggregator 汇总结果
//! - Hierarchical — supervisor 协调 workers

pub mod orchestrator;
pub mod patterns;

pub use orchestrator::{Orchestrator, OrchestratorBuilder};
pub use patterns::OrchestrationPattern;
