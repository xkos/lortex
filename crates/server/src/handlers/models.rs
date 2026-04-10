//! /v1/models — 按 API Key 返回可用模型列表

use axum::{extract::Extension, http::StatusCode, Json};

use crate::models::ApiKey;
use crate::proto::openai::{ErrorResponse, ModelObject, ModelsResponse};
use crate::state::AppState;

pub async fn list_models(
    Extension(state): Extension<AppState>,
    Extension(api_key): Extension<ApiKey>,
) -> Result<Json<ModelsResponse>, (StatusCode, Json<ErrorResponse>)> {
    let all_models = state
        .store
        .list_models()
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("Store error", "server_error")),
            )
        })?;

    // 过滤出 API Key 模型组中的模型
    let model_objects: Vec<ModelObject> = all_models
        .iter()
        .filter(|m| {
            m.enabled
                && api_key
                    .model_group
                    .iter()
                    .any(|name| m.matches(name))
        })
        .map(|m| ModelObject {
            id: m.id(),
            object: "model".into(),
            created: m.created_at.timestamp(),
            owned_by: m.provider_id.clone(),
        })
        .collect();

    Ok(Json(ModelsResponse {
        object: "list".into(),
        data: model_objects,
    }))
}
