//! Shared proxy handler logic — resolve_model, build_provider, map errors, fallback

use std::collections::HashMap;
use std::sync::Arc;

use axum::http::StatusCode;

use lortex_core::error::ProviderError;
use lortex_core::provider::{CompletionRequest, CompletionResponse, Provider};

use crate::handlers::provider_builder::build_llm_provider;
use crate::models::model::ApiFormat;
use crate::models::{ApiKey, Model};
use crate::state::AppState;

/// 从客户端请求中提取需要透传给上游 provider 的 headers
pub fn extract_passthrough_headers(headers: &axum::http::HeaderMap) -> HashMap<String, String> {
    // 透传的 header 前缀/名称列表
    const PASSTHROUGH_HEADERS: &[&str] = &[
        "anthropic-beta",
    ];

    let mut result = HashMap::new();
    for name in PASSTHROUGH_HEADERS {
        if let Some(value) = headers.get(*name) {
            if let Ok(v) = value.to_str() {
                result.insert(name.to_string(), v.to_string());
            }
        }
    }
    result
}

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
    build_provider_with_headers(state, model, preferred_format, &HashMap::new()).await
}

/// 根据 Provider 配置构建 lortex Provider 实例，合并客户端 headers
pub async fn build_provider_with_headers(
    state: &AppState,
    model: &Model,
    preferred_format: &ApiFormat,
    client_headers: &HashMap<String, String>,
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

    Ok(build_llm_provider(&provider_config, model, preferred_format, client_headers))
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

/// 判断 ProviderError 是否可以 fallback（网络/服务器错误可以，认证/请求错误不行）
fn is_retriable(e: &ProviderError) -> bool {
    match e {
        ProviderError::Network(_) | ProviderError::RateLimited { .. } => true,
        ProviderError::Api { status, .. } => *status >= 500,
        _ => false,
    }
}

/// 解析主模型 + fallback 模型列表（按优先级），过滤不可用的 provider
///
/// 当主模型设置了 rpm_limit / tpm_limit 时，自动将 api_key.model_group
/// 中同类型的其他模型追加为降级候选。
pub async fn resolve_models_with_fallback(
    state: &AppState,
    api_key: &ApiKey,
    model_name: &str,
) -> Result<Vec<Model>, ProxyError> {
    let primary = resolve_model(state, api_key, model_name).await?;
    let has_model_rate_limit = primary.rpm_limit > 0 || primary.tpm_limit > 0;
    let mut models = vec![primary.clone()];

    for fallback_name in &api_key.fallback_models {
        if let Ok(m) = resolve_model(state, api_key, fallback_name).await {
            // 避免重复
            if !models.iter().any(|existing| existing.id() == m.id()) {
                models.push(m);
            }
        }
    }

    // 如果主模型有限流，追加 model_group 中同类型的其他模型作为降级候选
    if has_model_rate_limit {
        for name in &api_key.model_group {
            if let Ok(m) = resolve_model(state, api_key, name).await {
                if m.model_type == primary.model_type
                    && !models.iter().any(|existing| existing.id() == m.id())
                {
                    models.push(m);
                }
            }
        }
    }

    Ok(models)
}

/// Non-streaming 带 fallback 的完整调用链
///
/// 依次尝试主模型和 fallback 模型，跳过熔断的 provider，
/// 成功/失败记录到 circuit breaker。
pub async fn complete_with_fallback(
    state: &AppState,
    api_key: &ApiKey,
    model_name: &str,
    preferred_format: &ApiFormat,
    client_headers: &HashMap<String, String>,
    mut build_request: impl FnMut(&Model) -> CompletionRequest,
) -> Result<(CompletionResponse, Model), ProxyError> {
    let models = resolve_models_with_fallback(state, api_key, model_name).await?;
    let mut last_error = None;

    for model in &models {
        // 检查熔断器
        let available = state
            .circuit_breaker
            .is_available(&model.provider_id)
            .await
            .unwrap_or(true); // store 错误时不阻塞
        if !available {
            tracing::info!(
                provider = %model.provider_id,
                model = %model.id(),
                "Skipping circuit-broken provider"
            );
            continue;
        }

        // 检查模型级 RPM 限流
        if model.rpm_limit > 0 {
            if state.rate_limiter.check_model_rpm(&model.id(), model.rpm_limit).is_err() {
                tracing::info!(model = %model.id(), "Skipping RPM-limited model");
                continue;
            }
        }
        // 检查模型级 TPM 限流
        if model.tpm_limit > 0 {
            if state.rate_limiter.check_model_tpm(&model.id(), model.tpm_limit).is_err() {
                tracing::info!(model = %model.id(), "Skipping TPM-limited model");
                continue;
            }
        }

        // 构建 provider
        let provider = match build_provider_with_headers(state, model, preferred_format, client_headers).await {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    model = %model.id(),
                    error = %e.message,
                    "Failed to build provider, trying next"
                );
                last_error = Some(e);
                continue;
            }
        };

        // 发起请求
        let request = build_request(model);
        match provider.complete(request).await {
            Ok(resp) => {
                // 记录成功
                let _ = state.circuit_breaker.record_success(&model.provider_id).await;
                // 记录模型级 RPM
                state.rate_limiter.record_model_request(&model.id());
                return Ok((resp, model.clone()));
            }
            Err(e) => {
                tracing::warn!(
                    model = %model.id(),
                    provider = %model.provider_id,
                    error = %e,
                    "LLM call failed, checking fallback"
                );
                // 记录失败
                let _ = state.circuit_breaker.record_failure(&model.provider_id).await;

                if is_retriable(&e) && models.len() > 1 {
                    last_error = Some(map_provider_error(e));
                    continue;
                }
                return Err(map_provider_error(e));
            }
        }
    }

    Err(last_error.unwrap_or_else(|| {
        ProxyError::unavailable("All models unavailable (circuit-broken or failed)")
    }))
}
