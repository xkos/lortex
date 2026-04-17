//! Admin API — Provider 健康状态 & 熔断管理

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};

use crate::models::ProviderHealthStatus;
use crate::state::AppState;

pub async fn list(
    State(state): State<AppState>,
) -> Result<Json<Vec<ProviderHealthStatus>>, StatusCode> {
    state
        .store
        .list_health_statuses()
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn reset(
    State(state): State<AppState>,
    Path(provider_id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    state
        .circuit_breaker
        .force_reset(&provider_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}
