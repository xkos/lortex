//! 应用共享状态

use std::sync::Arc;

use crate::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use crate::store::ProxyStore;

/// 应用共享状态，注入到 axum handlers
#[derive(Clone)]
pub struct AppState {
    pub store: Arc<dyn ProxyStore>,
    pub circuit_breaker: Arc<CircuitBreaker>,
}

impl AppState {
    /// 使用默认 CircuitBreaker 配置创建 AppState
    pub fn new(store: Arc<dyn ProxyStore>) -> Self {
        let cb = CircuitBreaker::new(store.clone(), CircuitBreakerConfig::default());
        Self {
            store,
            circuit_breaker: Arc::new(cb),
        }
    }
}
