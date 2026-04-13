//! Admin API — Provider 管理

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::Deserialize;

use crate::models::provider::{Provider, Vendor};
use crate::state::AppState;

#[derive(Deserialize)]
pub struct CreateProviderRequest {
    pub id: String,
    pub vendor: String,
    pub display_name: String,
    pub api_key: String,
    pub base_url: String,
    #[serde(default)]
    pub website_url: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool { true }

#[derive(Deserialize)]
pub struct UpdateProviderRequest {
    pub vendor: Option<String>,
    pub display_name: Option<String>,
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    pub website_url: Option<String>,
    pub enabled: Option<bool>,
}

pub async fn list(
    State(state): State<AppState>,
) -> Result<Json<Vec<Provider>>, StatusCode> {
    state.store.list_providers().await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Provider>, StatusCode> {
    state.store.get_provider(&id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateProviderRequest>,
) -> Result<(StatusCode, Json<Provider>), StatusCode> {
    let provider = Provider {
        id: req.id,
        vendor: Vendor::from_str(&req.vendor),
        display_name: req.display_name,
        api_key: req.api_key,
        base_url: req.base_url,
        website_url: req.website_url,
        enabled: req.enabled,
        created_at: Utc::now(),
    };
    state.store.upsert_provider(&provider).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((StatusCode::CREATED, Json(provider)))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateProviderRequest>,
) -> Result<Json<Provider>, StatusCode> {
    let mut provider = state.store.get_provider(&id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if let Some(v) = req.vendor { provider.vendor = Vendor::from_str(&v); }
    if let Some(v) = req.display_name { provider.display_name = v; }
    if let Some(v) = req.api_key { provider.api_key = v; }
    if let Some(v) = req.base_url { provider.base_url = v; }
    if let Some(v) = req.website_url { provider.website_url = v; }
    if let Some(v) = req.enabled { provider.enabled = v; }

    state.store.upsert_provider(&provider).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(provider))
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    state.store.delete_provider(&id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}
