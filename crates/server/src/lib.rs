//! lortex-server: Proxy 服务层
//!
//! 提供 LLM 中转代理的 HTTP 服务：
//! - 数据模型（Provider、Model、ApiKey）
//! - 可插拔存储（ProxyStore trait + SQLite 默认实现）
//! - Admin 管理 API
//! - 代理入口（OpenAI / Anthropic 格式）

pub mod config;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod routes;
pub mod state;
pub mod store;

pub use config::ServerConfig;
pub use routes::app_router;
pub use state::AppState;
