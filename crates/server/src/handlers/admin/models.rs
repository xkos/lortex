//! Admin API — Model 管理

use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chrono::Utc;
use serde::{Deserialize, Deserializer};

use crate::models::model::{ApiFormat, Model, ModelType};
use crate::state::AppState;

fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(|opt| opt.unwrap_or_default())
}

#[derive(Deserialize)]
pub struct CreateModelRequest {
    pub provider_id: String,
    pub vendor_model_name: String,
    pub display_name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default = "default_chat")]
    pub model_type: String,
    #[serde(default = "default_api_formats")]
    pub api_formats: Vec<String>,

    #[serde(default = "default_true")]
    pub supports_streaming: bool,
    #[serde(default)]
    pub supports_tools: bool,
    #[serde(default)]
    pub supports_structured_output: bool,
    #[serde(default)]
    pub supports_vision: bool,
    #[serde(default)]
    pub supports_prefill: bool,
    #[serde(default)]
    pub supports_cache: bool,
    #[serde(default)]
    pub supports_web_search: bool,
    #[serde(default)]
    pub supports_batch: bool,
    #[serde(default)]
    pub context_window: u32,

    #[serde(default = "default_true")]
    pub cache_enabled: bool,
    #[serde(default = "default_cache_strategy")]
    pub cache_strategy: String,

    #[serde(default, deserialize_with = "null_as_default")]
    pub extra_headers: HashMap<String, String>,

    #[serde(default)]
    pub rpm_limit: u32,
    #[serde(default)]
    pub tpm_limit: u32,

    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool { true }
fn default_chat() -> String { "chat".into() }
fn default_api_formats() -> Vec<String> { vec!["openai".into()] }
fn default_cache_strategy() -> String { "full".into() }

pub async fn list(
    State(state): State<AppState>,
) -> Result<Json<Vec<Model>>, StatusCode> {
    state.store.list_models().await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn get(
    State(state): State<AppState>,
    Path((provider_id, model_name)): Path<(String, String)>,
) -> Result<Json<Model>, StatusCode> {
    state.store.get_model(&provider_id, &model_name).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn create(
    State(state): State<AppState>,
    Json(req): Json<CreateModelRequest>,
) -> Result<(StatusCode, Json<Model>), StatusCode> {
    let model = Model {
        provider_id: req.provider_id,
        vendor_model_name: req.vendor_model_name,
        display_name: req.display_name,
        aliases: req.aliases,
        model_type: ModelType::from_str(&req.model_type),
        api_formats: req.api_formats.iter().map(|s| ApiFormat::from_str(s)).collect(),
        supports_streaming: req.supports_streaming,
        supports_tools: req.supports_tools,
        supports_structured_output: req.supports_structured_output,
        supports_vision: req.supports_vision,
        supports_prefill: req.supports_prefill,
        supports_cache: req.supports_cache,
        supports_web_search: req.supports_web_search,
        supports_batch: req.supports_batch,
        context_window: req.context_window,
        cache_enabled: req.cache_enabled,
        cache_strategy: req.cache_strategy,
        extra_headers: req.extra_headers,
        rpm_limit: req.rpm_limit,
        tpm_limit: req.tpm_limit,
        enabled: req.enabled,
        created_at: Utc::now(),
    };
    state.store.upsert_model(&model).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((StatusCode::CREATED, Json(model)))
}

#[derive(Deserialize)]
pub struct UpdateModelRequest {
    pub display_name: Option<String>,
    pub aliases: Option<Vec<String>>,
    pub api_formats: Option<Vec<String>>,
    pub supports_streaming: Option<bool>,
    pub supports_tools: Option<bool>,
    pub supports_structured_output: Option<bool>,
    pub supports_vision: Option<bool>,
    pub supports_prefill: Option<bool>,
    pub supports_cache: Option<bool>,
    pub supports_web_search: Option<bool>,
    pub supports_batch: Option<bool>,
    pub context_window: Option<u32>,
    pub cache_enabled: Option<bool>,
    pub cache_strategy: Option<String>,
    pub extra_headers: Option<HashMap<String, String>>,
    pub rpm_limit: Option<u32>,
    pub tpm_limit: Option<u32>,
    pub enabled: Option<bool>,
}

pub async fn update(
    State(state): State<AppState>,
    Path((provider_id, model_name)): Path<(String, String)>,
    Json(req): Json<UpdateModelRequest>,
) -> Result<Json<Model>, StatusCode> {
    let mut model = state.store.get_model(&provider_id, &model_name).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if let Some(v) = req.display_name { model.display_name = v; }
    if let Some(v) = req.aliases { model.aliases = v; }
    if let Some(v) = req.api_formats { model.api_formats = v.iter().map(|s| ApiFormat::from_str(s)).collect(); }
    if let Some(v) = req.supports_streaming { model.supports_streaming = v; }
    if let Some(v) = req.supports_tools { model.supports_tools = v; }
    if let Some(v) = req.supports_structured_output { model.supports_structured_output = v; }
    if let Some(v) = req.supports_vision { model.supports_vision = v; }
    if let Some(v) = req.supports_prefill { model.supports_prefill = v; }
    if let Some(v) = req.supports_cache { model.supports_cache = v; }
    if let Some(v) = req.supports_web_search { model.supports_web_search = v; }
    if let Some(v) = req.supports_batch { model.supports_batch = v; }
    if let Some(v) = req.context_window { model.context_window = v; }
    if let Some(v) = req.cache_enabled { model.cache_enabled = v; }
    if let Some(v) = req.cache_strategy { model.cache_strategy = v; }
    if let Some(v) = req.extra_headers { model.extra_headers = v; }
    if let Some(v) = req.rpm_limit { model.rpm_limit = v; }
    if let Some(v) = req.tpm_limit { model.tpm_limit = v; }
    if let Some(v) = req.enabled { model.enabled = v; }

    state.store.upsert_model(&model).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(model))
}

pub async fn delete(
    State(state): State<AppState>,
    Path((provider_id, model_name)): Path<(String, String)>,
) -> Result<StatusCode, StatusCode> {
    state.store.delete_model(&provider_id, &model_name).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}
