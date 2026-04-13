//! 应用共享状态

use std::sync::Arc;

use crate::circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
use crate::rate_limiter::RateLimiter;
use crate::store::ProxyStore;

/// 应用共享状态，注入到 axum handlers
#[derive(Clone)]
pub struct AppState {
    pub store: Arc<dyn ProxyStore>,
    pub circuit_breaker: Arc<CircuitBreaker>,
    pub rate_limiter: Arc<RateLimiter>,
}

impl AppState {
    /// 使用默认 CircuitBreaker 配置创建 AppState
    pub fn new(store: Arc<dyn ProxyStore>) -> Self {
        Self::with_rate_limiter(store, Arc::new(RateLimiter::new()))
    }

    /// 使用外部 RateLimiter 创建 AppState（UsageLayer 需共享同一实例）
    pub fn with_rate_limiter(
        store: Arc<dyn ProxyStore>,
        rate_limiter: Arc<RateLimiter>,
    ) -> Self {
        let cb = CircuitBreaker::new(store.clone(), CircuitBreakerConfig::default());
        Self {
            store,
            circuit_breaker: Arc::new(cb),
            rate_limiter,
        }
    }
}
