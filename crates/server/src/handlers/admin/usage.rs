//! Admin API — 用量查询

use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use serde::Deserialize;

use crate::state::AppState;
use crate::store::traits::{GroupedUsage, TrendPoint, UsageQuery, UsageSummary};

#[derive(Deserialize)]
pub struct UsageQueryParams {
    pub api_key_id: Option<String>,
    pub provider_id: Option<String>,
    pub vendor_model_name: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub limit: Option<usize>,
}

impl UsageQueryParams {
    fn to_query(&self) -> Result<UsageQuery, String> {
        let start_time = self
            .start_time
            .as_deref()
            .map(|s| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .map_err(|e| format!("Invalid start_time: {}", e))
            })
            .transpose()?;
        let end_time = self
            .end_time
            .as_deref()
            .map(|s| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .map_err(|e| format!("Invalid end_time: {}", e))
            })
            .transpose()?;

        Ok(UsageQuery {
            api_key_id: self.api_key_id.clone(),
            provider_id: self.provider_id.clone(),
            vendor_model_name: self.vendor_model_name.clone(),
            start_time,
            end_time,
            limit: self.limit,
        })
    }
}

/// GET /admin/api/v1/usage — 查询用量记录
pub async fn list(
    State(state): State<AppState>,
    Json(params): Json<UsageQueryParams>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let query = params.to_query().map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let records = state
        .store
        .query_usage(&query)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(serde_json::to_value(&records).unwrap()))
}

/// GET /admin/api/v1/usage/summary — 用量汇总
pub async fn summary(
    State(state): State<AppState>,
    Json(params): Json<UsageQueryParams>,
) -> Result<Json<UsageSummary>, (StatusCode, String)> {
    let query = params.to_query().map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let summary = state
        .store
        .summarize_usage(&query)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(summary))
}

/// POST /admin/api/v1/usage/trend — 时间趋势（按日分桶）
pub async fn trend(
    State(state): State<AppState>,
    Json(params): Json<UsageQueryParams>,
) -> Result<Json<Vec<TrendPoint>>, (StatusCode, String)> {
    let query = params.to_query().map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let points = state
        .store
        .usage_trend(&query)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(points))
}

/// POST /admin/api/v1/usage/by-model — 按模型分组
pub async fn by_model(
    State(state): State<AppState>,
    Json(params): Json<UsageQueryParams>,
) -> Result<Json<Vec<GroupedUsage>>, (StatusCode, String)> {
    let query = params.to_query().map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let groups = state
        .store
        .usage_by_model(&query)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(groups))
}

/// POST /admin/api/v1/usage/by-key — 按 ApiKey 分组
pub async fn by_key(
    State(state): State<AppState>,
    Json(params): Json<UsageQueryParams>,
) -> Result<Json<Vec<GroupedUsage>>, (StatusCode, String)> {
    let query = params.to_query().map_err(|e| (StatusCode::BAD_REQUEST, e))?;
    let groups = state
        .store
        .usage_by_key(&query)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(groups))
}
