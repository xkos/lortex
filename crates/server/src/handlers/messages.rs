//! /v1/messages — Anthropic Messages API 兼容入口

use std::sync::Arc;

use axum::{extract::Extension, http::StatusCode, Json};

use lortex_core::error::ProviderError;
use lortex_core::provider::Provider;

use crate::middleware::proxy_auth::deduct_credits;
use crate::models::{ApiKey, Model};
use crate::models::provider::Vendor;
use crate::proto::anthropic::{AnthropicError, MessagesRequest, MessagesResponse};
use crate::proto::convert::{anthropic_request_to_lortex, lortex_response_to_anthropic};
use crate::state::AppState;

/// 解析模型（复用 chat handler 的逻辑）
async fn resolve_model(
    state: &AppState,
    api_key: &ApiKey,
    model_name: &str,
) -> Result<Model, (StatusCode, Json<AnthropicError>)> {
    let effective_name = if model_name == "PROXY_MANAGED" {
        &api_key.default_model
    } else {
        model_name
    };

    let model = state
        .store
        .find_model(effective_name)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AnthropicError::new("error", "api_error", "Store error")),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(AnthropicError::not_found(format!(
                    "Model '{}' not found",
                    effective_name
                ))),
            )
        })?;

    if !api_key.model_group.iter().any(|name| model.matches(name)) {
        return Err((
            StatusCode::NOT_FOUND,
            Json(AnthropicError::not_found(format!(
                "Model '{}' not available for this API key",
                effective_name
            ))),
        ));
    }

    if !model.enabled {
        return Err((
            StatusCode::NOT_FOUND,
            Json(AnthropicError::not_found(format!(
                "Model '{}' is disabled",
                effective_name
            ))),
        ));
    }

    Ok(model)
}

/// 构建 provider
async fn build_provider(
    state: &AppState,
    model: &Model,
) -> Result<Arc<dyn Provider>, (StatusCode, Json<AnthropicError>)> {
    let provider_config = state
        .store
        .get_provider(&model.provider_id)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AnthropicError::new("error", "api_error", "Store error")),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AnthropicError::new(
                    "error",
                    "api_error",
                    format!("Provider '{}' not found", model.provider_id),
                )),
            )
        })?;

    if !provider_config.enabled {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(AnthropicError::new(
                "error",
                "api_error",
                format!("Provider '{}' is disabled", model.provider_id),
            )),
        ));
    }

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

/// POST /v1/messages — Anthropic Messages API（non-streaming）
pub async fn messages(
    Extension(state): Extension<AppState>,
    Extension(api_key): Extension<ApiKey>,
    Json(req): Json<MessagesRequest>,
) -> Result<Json<MessagesResponse>, (StatusCode, Json<AnthropicError>)> {
    // TODO: streaming support in 003c continuation or later
    if req.stream {
        return Err((
            StatusCode::NOT_IMPLEMENTED,
            Json(AnthropicError::invalid_request(
                "Streaming not yet supported for /v1/messages",
            )),
        ));
    }

    let model = resolve_model(&state, &api_key, &req.model).await?;
    tracing::info!(
        key_name = %api_key.name,
        requested_model = %req.model,
        resolved_model = %model.id(),
        provider = %model.provider_id,
        endpoint = "/v1/messages",
        "Routing Anthropic messages request"
    );

    let provider = build_provider(&state, &model).await?;

    let mut lortex_req = anthropic_request_to_lortex(&req);
    lortex_req.model = model.vendor_model_name.clone();

    let start = std::time::Instant::now();
    let lortex_resp = provider.complete(lortex_req).await.map_err(|e| {
        tracing::error!(
            provider = %model.provider_id,
            model = %model.vendor_model_name,
            error = %e,
            "Upstream LLM call failed"
        );
        let (status, msg) = match &e {
            ProviderError::RateLimited { .. } => {
                (StatusCode::TOO_MANY_REQUESTS, e.to_string())
            }
            ProviderError::AuthenticationFailed(_) => {
                (StatusCode::UNAUTHORIZED, "Upstream authentication failed".into())
            }
            _ => (StatusCode::BAD_GATEWAY, e.to_string()),
        };
        (status, Json(AnthropicError::new("error", "api_error", msg)))
    })?;
    let elapsed = start.elapsed();

    if let Some(usage) = &lortex_resp.usage {
        let credits = deduct_credits(
            &state, &api_key, &model,
            usage.prompt_tokens, usage.completion_tokens, 0, 0,
        ).await.unwrap_or(0);

        tracing::info!(
            key_name = %api_key.name,
            model = %model.id(),
            input_tokens = usage.prompt_tokens,
            output_tokens = usage.completion_tokens,
            credits_deducted = credits,
            elapsed_ms = elapsed.as_millis() as u64,
            "Anthropic messages request done"
        );
    }

    let anthropic_resp = lortex_response_to_anthropic(&lortex_resp, &model.id());
    Ok(Json(anthropic_resp))
}
