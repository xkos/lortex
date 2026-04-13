//! Admin API — ApiKey 管理

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::Deserialize;

use crate::models::api_key::ApiKey;
use crate::state::AppState;

#[derive(Deserialize)]
pub struct CreateApiKeyRequest {
    pub name: String,
    #[serde(default)]
    pub model_group: Vec<String>,
    #[serde(default)]
    pub default_model: String,
    #[serde(default)]
    pub fallback_models: Vec<String>,
    #[serde(default)]
    pub credit_limit: i64,
    #[serde(default)]
    pub rpm_limit: u32,
    #[serde(default)]
    pub tpm_limit: u32,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool { true }

#[derive(Deserialize)]
pub struct UpdateApiKeyRequest {
    pub name: Option<String>,
    pub model_group: Option<Vec<String>>,
    pub default_model: Option<String>,
    pub fallback_models: Option<Vec<String>>,
    pub credit_limit: Option<i64>,
    pub rpm_limit: Option<u32>,
    pub tpm_limit: Option<u32>,
    pub enabled: Option<bool>,
}

/// API Key 列表响应（隐藏完整 key，只显示前缀）
#[derive(serde::Serialize)]
pub struct ApiKeyResponse {
    pub id: String,
    pub key_prefix: String,
    pub name: String,
    pub model_group: Vec<String>,
    pub default_model: String,
    pub credit_limit: i64,
    pub credit_used: i64,
    pub rpm_limit: u32,
    pub tpm_limit: u32,
    pub enabled: bool,
}

impl From<&ApiKey> for ApiKeyResponse {
    fn from(k: &ApiKey) -> Self {
        let key_prefix = if k.key.len() > 12 {
            format!("{}...", &k.key[..12])
        } else {
            k.key.clone()
        };
        Self {
            id: k.id.clone(),
            key_prefix,
            name: k.name.clone(),
            model_group: k.model_group.clone(),
            default_model: k.default_model.clone(),
            credit_limit: k.credit_limit,
            credit_used: k.credit_used,
            rpm_limit: k.rpm_limit,
            tpm_limit: k.tpm_limit,
            enabled: k.enabled,
        }
    }
}

pub async fn list(
    State(state): State<AppState>,
) -> Result<Json<Vec<ApiKeyResponse>>, StatusCode> {
    let keys = state.store.list_api_keys().await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(keys.iter().map(ApiKeyResponse::from).collect()))
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ApiKeyResponse>, StatusCode> {
    let key = state.store.get_api_key_by_id(&id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(ApiKeyResponse::from(&key)))
}

/// 创建 ApiKey — 返回完整 key（仅此一次可见）
pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateApiKeyRequest>,
) -> Result<(StatusCode, Json<ApiKey>), StatusCode> {
    let api_key = ApiKey {
        id: uuid::Uuid::new_v4().to_string(),
        key: ApiKey::generate_key(),
        name: req.name,
        model_group: req.model_group,
        default_model: req.default_model,
        fallback_models: req.fallback_models,
        credit_limit: req.credit_limit,
        credit_used: 0,
        rpm_limit: req.rpm_limit,
        tpm_limit: req.tpm_limit,
        enabled: req.enabled,
        created_at: Utc::now(),
        last_used_at: None,
    };
    state.store.upsert_api_key(&api_key).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    // 创建时返回完整 key，后续 list/get 只返回前缀
    Ok((StatusCode::CREATED, Json(api_key)))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateApiKeyRequest>,
) -> Result<Json<ApiKeyResponse>, StatusCode> {
    let mut key = state.store.get_api_key_by_id(&id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if let Some(v) = req.name { key.name = v; }
    if let Some(v) = req.model_group { key.model_group = v; }
    if let Some(v) = req.default_model { key.default_model = v; }
    if let Some(v) = req.fallback_models { key.fallback_models = v; }
    if let Some(v) = req.credit_limit { key.credit_limit = v; }
    if let Some(v) = req.rpm_limit { key.rpm_limit = v; }
    if let Some(v) = req.tpm_limit { key.tpm_limit = v; }
    if let Some(v) = req.enabled { key.enabled = v; }

    state.store.upsert_api_key(&key).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(ApiKeyResponse::from(&key)))
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    state.store.delete_api_key(&id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn reset_credits(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    state.store.reset_credits(&id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::OK)
}

/// 获取完整 API Key（admin only）
pub async fn reveal_key(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let key = state.store.get_api_key_by_id(&id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(serde_json::json!({"key": key.key})))
}
