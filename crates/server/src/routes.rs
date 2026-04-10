//! HTTP 路由构建

use axum::{
    middleware,
    routing::{get, post},
    Router,
};

use crate::handlers::admin;
use crate::middleware::auth::{admin_auth, AdminKey};
use crate::state::AppState;

/// 构建 admin API 路由
pub fn admin_routes(state: AppState, admin_key: String) -> Router {
    Router::new()
        .route("/providers", get(admin::providers::list).post(admin::providers::create))
        .route("/providers/{id}", get(admin::providers::get).put(admin::providers::update).delete(admin::providers::delete))
        .route("/models", get(admin::models::list).post(admin::models::create))
        .route("/models/{provider_id}/{model_name}", get(admin::models::get).delete(admin::models::delete))
        .route("/keys", get(admin::keys::list).post(admin::keys::create))
        .route("/keys/{id}", get(admin::keys::get).put(admin::keys::update).delete(admin::keys::delete))
        .route("/keys/{id}/reset-credits", post(admin::keys::reset_credits))
        .layer(middleware::from_fn(admin_auth))
        .layer(axum::Extension(AdminKey(admin_key)))
        .with_state(state)
}

/// 构建完整的应用路由
pub fn app_router(state: AppState, admin_key: String) -> Router {
    Router::new()
        .nest("/admin/v1", admin_routes(state, admin_key))
}
