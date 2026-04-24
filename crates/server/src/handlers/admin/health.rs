//! Admin API — 模型健康状态 & 熔断管理

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::handlers::admin::LogInternal;
use crate::models::ModelHealthStatus;
use crate::state::AppState;

pub async fn list(
    State(state): State<AppState>,
) -> Result<Json<Vec<ModelHealthStatus>>, StatusCode> {
    state
        .store
        .list_health_statuses()
        .await
        .map(Json)
        .log_internal("list_health_statuses")
}

pub async fn reset(
    State(state): State<AppState>,
    Path((provider_id, model_name)): Path<(String, String)>,
) -> Result<StatusCode, StatusCode> {
    let model_id = format!("{provider_id}/{model_name}");
    state
        .circuit_breaker
        .force_reset(&model_id)
        .await
        .log_internal("circuit_breaker.force_reset")?;
    Ok(StatusCode::NO_CONTENT)
}
