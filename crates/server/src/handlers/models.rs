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
    let mut model_objects: Vec<ModelObject> = all_models
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

    if !api_key.default_model.is_empty() {
        model_objects.insert(
            0,
            ModelObject {
                id: "PROXY_MANAGED".into(),
                object: "model".into(),
                created: chrono::Utc::now().timestamp(),
                owned_by: "proxy".into(),
            },
        );
    }

    // model_map 中的客户端别名也加入列表，使客户端验证通过
    let existing_ids: std::collections::HashSet<String> =
        model_objects.iter().map(|m| m.id.clone()).collect();
    for (alias, target) in &api_key.model_map {
        if existing_ids.contains(alias) {
            continue;
        }
        let owned_by = all_models
            .iter()
            .find(|m| m.id() == *target || m.matches(target))
            .map(|m| m.provider_id.clone())
            .unwrap_or_else(|| "proxy".into());
        model_objects.push(ModelObject {
            id: alias.clone(),
            object: "model".into(),
            created: chrono::Utc::now().timestamp(),
            owned_by,
        });
    }

    Ok(Json(ModelsResponse {
        object: "list".into(),
        data: model_objects,
    }))
}
