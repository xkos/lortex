//! 应用共享状态

use std::sync::Arc;

use crate::store::ProxyStore;

/// 应用共享状态，注入到 axum handlers
#[derive(Clone)]
pub struct AppState {
    pub store: Arc<dyn ProxyStore>,
}
