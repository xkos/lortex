//! 中间件

pub mod auth;
pub mod proxy_auth;

pub use auth::AdminKey;
pub use proxy_auth::{deduct_credits, proxy_auth};
