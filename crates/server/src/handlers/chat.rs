//! /v1/chat/completions — OpenAI 兼容对话补全

use std::sync::Arc;

use axum::{extract::Extension, http::StatusCode, Json};

use lortex_core::error::ProviderError;
use lortex_core::provider::Provider;

use crate::middleware::proxy_auth::deduct_credits;
use crate::models::{ApiKey, Model};
use crate::models::provider::Vendor;
use crate::proto::convert::{openai_request_to_lortex, lortex_response_to_openai};
use crate::proto::openai::{ChatCompletionRequest, ChatCompletionResponse, ErrorResponse};
use crate::state::AppState;

/// 解析模型：PROXY_MANAGED / 精确 ID / 别名
async fn resolve_model(
    state: &AppState,
    api_key: &ApiKey,
    model_name: &str,
) -> Result<Model, (StatusCode, Json<ErrorResponse>)> {
    let effective_name = if model_name == "PROXY_MANAGED" {
        &api_key.default_model
    } else {
        model_name
    };

    // 从 store 查找模型
    let model = state
        .store
        .find_model(effective_name)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("Store error", "server_error")),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::not_found(format!(
                    "Model '{}' not found",
                    effective_name
                ))),
            )
        })?;

    // 检查模型是否在 API Key 的模型组中
    if !api_key.model_group.iter().any(|name| model.matches(name)) {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::not_found(format!(
                "Model '{}' not available for this API key",
                effective_name
            ))),
        ));
    }

    if !model.enabled {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::not_found(format!(
                "Model '{}' is disabled",
                effective_name
            ))),
        ));
    }

    Ok(model)
}

/// 根据 Provider 配置构建 lortex Provider 实例
async fn build_provider(
    state: &AppState,
    model: &Model,
) -> Result<Arc<dyn Provider>, (StatusCode, Json<ErrorResponse>)> {
    let provider_config = state
        .store
        .get_provider(&model.provider_id)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("Store error", "server_error")),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    format!("Provider '{}' not found", model.provider_id),
                    "server_error",
                )),
            )
        })?;

    if !provider_config.enabled {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse::new(
                format!("Provider '{}' is disabled", model.provider_id),
                "server_error",
            )),
        ));
    }

    // 根据 vendor 类型构建对应的 Provider
    let provider: Arc<dyn Provider> = match &provider_config.vendor {
        Vendor::OpenAI | Vendor::DeepSeek | Vendor::Custom(_) => {
            Arc::new(
                lortex_providers::openai::OpenAIProvider::new(&provider_config.api_key)
                    .with_base_url(&provider_config.base_url),
            )
        }
        Vendor::Anthropic => {
            Arc::new(
                lortex_providers::anthropic::AnthropicProvider::new(&provider_config.api_key)
                    .with_base_url(&provider_config.base_url),
            )
        }
    };

    Ok(provider)
}

/// POST /v1/chat/completions（non-streaming）
pub async fn chat_completions(
    Extension(state): Extension<AppState>,
    Extension(api_key): Extension<ApiKey>,
    Json(req): Json<ChatCompletionRequest>,
) -> Result<Json<ChatCompletionResponse>, (StatusCode, Json<ErrorResponse>)> {
    // 1. 解析模型
    let model = resolve_model(&state, &api_key, &req.model).await?;

    // 2. 构建 provider
    let provider = build_provider(&state, &model).await?;

    // 3. 转换请求
    let mut lortex_req = openai_request_to_lortex(&req);
    lortex_req.model = model.vendor_model_name.clone();

    // 4. 调用 LLM
    let lortex_resp = provider.complete(lortex_req).await.map_err(|e| {
        let (status, msg) = match &e {
            ProviderError::RateLimited { .. } => {
                (StatusCode::TOO_MANY_REQUESTS, e.to_string())
            }
            ProviderError::AuthenticationFailed(_) => {
                (StatusCode::UNAUTHORIZED, "Upstream authentication failed".into())
            }
            _ => (StatusCode::BAD_GATEWAY, e.to_string()),
        };
        (status, Json(ErrorResponse::new(msg, "upstream_error")))
    })?;

    // 5. 扣减 credit
    if let Some(usage) = &lortex_resp.usage {
        let _ = deduct_credits(&state, &api_key, &model, usage.prompt_tokens, usage.completion_tokens, 0, 0).await;
    }

    // 6. 转换响应
    let oai_resp = lortex_response_to_openai(&lortex_resp, &model.id());
    Ok(Json(oai_resp))
}
