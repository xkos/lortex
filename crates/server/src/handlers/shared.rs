//! Shared proxy handler logic — resolve_model, build_provider, map errors

use std::sync::Arc;

use axum::http::StatusCode;

use lortex_core::error::ProviderError;
use lortex_core::provider::Provider;

use crate::handlers::provider_builder::build_llm_provider;
use crate::models::model::ApiFormat;
use crate::models::{ApiKey, Model};
use crate::state::AppState;

/// Generic proxy error before converting to format-specific responses.
#[derive(Debug)]
pub struct ProxyError {
    pub status: StatusCode,
    pub message: String,
}

impl ProxyError {
    pub fn internal(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::INTERNAL_SERVER_ERROR, message: msg.into() }
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::NOT_FOUND, message: msg.into() }
    }
    pub fn unavailable(msg: impl Into<String>) -> Self {
        Self { status: StatusCode::SERVICE_UNAVAILABLE, message: msg.into() }
    }
}

/// 解析模型：PROXY_MANAGED / 精确 ID / 别名
pub async fn resolve_model(
    state: &AppState,
    api_key: &ApiKey,
    model_name: &str,
) -> Result<Model, ProxyError> {
    let effective_name = if model_name == "PROXY_MANAGED" {
        &api_key.default_model
    } else {
        model_name
    };

    let model = state
        .store
        .find_model(effective_name)
        .await
        .map_err(|_| ProxyError::internal("Store error"))?
        .ok_or_else(|| ProxyError::not_found(format!("Model '{}' not found", effective_name)))?;

    if !api_key.model_group.iter().any(|name| model.matches(name)) {
        return Err(ProxyError::not_found(format!(
            "Model '{}' not available for this API key",
            effective_name
        )));
    }

    if !model.enabled {
        return Err(ProxyError::not_found(format!(
            "Model '{}' is disabled",
            effective_name
        )));
    }

    Ok(model)
}

/// 根据 Provider 配置构建 lortex Provider 实例
pub async fn build_provider(
    state: &AppState,
    model: &Model,
    preferred_format: &ApiFormat,
) -> Result<Arc<dyn Provider>, ProxyError> {
    let provider_config = state
        .store
        .get_provider(&model.provider_id)
        .await
        .map_err(|_| ProxyError::internal("Store error"))?
        .ok_or_else(|| {
            ProxyError::internal(format!("Provider '{}' not found", model.provider_id))
        })?;

    if !provider_config.enabled {
        return Err(ProxyError::unavailable(format!(
            "Provider '{}' is disabled",
            model.provider_id
        )));
    }

    Ok(build_llm_provider(&provider_config, model, preferred_format))
}

/// Map upstream provider error to ProxyError
pub fn map_provider_error(e: ProviderError) -> ProxyError {
    tracing::error!(error = %e, "Upstream LLM call failed");
    let (status, msg) = match &e {
        ProviderError::RateLimited { .. } => (StatusCode::TOO_MANY_REQUESTS, e.to_string()),
        ProviderError::AuthenticationFailed(_) => {
            (StatusCode::UNAUTHORIZED, "Upstream authentication failed".into())
        }
        _ => (StatusCode::BAD_GATEWAY, e.to_string()),
    };
    ProxyError { status, message: msg }
}
