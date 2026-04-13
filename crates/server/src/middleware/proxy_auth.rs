//! Proxy API Key 鉴权中间件

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};

use crate::models::ApiKey;
use crate::proto::openai::ErrorResponse;
use crate::state::AppState;

/// 从请求中提取 proxy API key，验证有效性和额度
pub async fn proxy_auth(
    request: Request,
    next: Next,
) -> Result<Response, Response> {
    let state = request
        .extensions()
        .get::<AppState>()
        .cloned()
        .ok_or_else(|| {
            (StatusCode::INTERNAL_SERVER_ERROR, "Server misconfigured").into_response()
        })?;

    // 提取 key：支持 OpenAI 格式和 Anthropic 格式
    let key_str = extract_api_key(&request).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse::auth_error("Missing API key")),
        )
            .into_response()
    })?;

    // 查找 key
    let api_key = state
        .store
        .get_api_key_by_key(&key_str)
        .await
        .map_err(|_| {
            (StatusCode::INTERNAL_SERVER_ERROR, "Store error").into_response()
        })?
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse::auth_error("Invalid API key")),
            )
                .into_response()
        })?;

    // 检查是否启用
    if !api_key.enabled {
        tracing::warn!(key_id = %api_key.id, key_name = %api_key.name, "API key is disabled");
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse::auth_error("API key is disabled")),
        )
            .into_response());
    }

    // 检查额度
    if !api_key.has_credits() {
        tracing::warn!(
            key_id = %api_key.id,
            key_name = %api_key.name,
            credit_used = api_key.credit_used,
            credit_limit = api_key.credit_limit,
            "Credit limit exceeded"
        );
        return Err((
            StatusCode::TOO_MANY_REQUESTS,
            Json(ErrorResponse::rate_limit("Credit limit exceeded")),
        )
            .into_response());
    }

    tracing::debug!(
        key_id = %api_key.id,
        key_name = %api_key.name,
        "API key authenticated"
    );

    // 注入 ApiKey 到 request extensions
    let mut request = request;
    request.extensions_mut().insert(api_key);

    Ok(next.run(request).await)
}

/// 从请求头提取 API key
/// 支持：Authorization: Bearer sk-proxy-xxx 和 x-api-key: sk-proxy-xxx
fn extract_api_key(request: &Request) -> Option<String> {
    // OpenAI 格式
    if let Some(auth) = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
    {
        if let Some(key) = auth.strip_prefix("Bearer ") {
            return Some(key.trim().to_string());
        }
    }

    // Anthropic 格式
    if let Some(key) = request
        .headers()
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
    {
        return Some(key.trim().to_string());
    }

    None
}

/// 请求完成后扣减 credit 并写入用量记录
pub async fn deduct_credits(
    state: &AppState,
    api_key: &ApiKey,
    model: &crate::models::Model,
    input_tokens: u32,
    output_tokens: u32,
    cache_write_tokens: u32,
    cache_read_tokens: u32,
    endpoint: &str,
    latency_ms: u64,
    estimated_chars: u64,
) -> Result<i64, crate::store::StoreError> {
    let mut credits: f64 = 0.0;

    // 文本 token 计费
    credits += (input_tokens as f64 / 1000.0) * model.input_multiplier;
    credits += (output_tokens as f64 / 1000.0) * model.output_multiplier;

    // 缓存 token 计费
    if let Some(cw) = model.cache_write_multiplier {
        credits += (cache_write_tokens as f64 / 1000.0) * cw;
    }
    if let Some(cr) = model.cache_read_multiplier {
        credits += (cache_read_tokens as f64 / 1000.0) * cr;
    }

    let credits_int = credits.ceil() as i64;
    state.store.add_credits_used(&api_key.id, credits_int).await?;

    // 写入用量记录
    let record = crate::models::UsageRecord {
        id: uuid::Uuid::new_v4().to_string(),
        api_key_id: api_key.id.clone(),
        api_key_name: api_key.name.clone(),
        provider_id: model.provider_id.clone(),
        vendor_model_name: model.vendor_model_name.clone(),
        request_endpoint: endpoint.to_string(),
        input_tokens,
        cache_write_tokens,
        cache_read_tokens,
        output_tokens,
        image_input_units: 0,
        audio_input_seconds: 0.0,
        credits_consumed: credits_int,
        estimated_chars,
        latency_ms,
        created_at: chrono::Utc::now(),
    };
    if let Err(e) = state.store.insert_usage(&record).await {
        tracing::warn!(error = %e, "Failed to write usage record");
    }

    Ok(credits_int)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use crate::models::model::{ApiFormat, Model, ModelType};
    use std::collections::HashMap;

    fn test_model() -> Model {
        Model {
            provider_id: "openai".into(),
            vendor_model_name: "gpt-4o".into(),
            display_name: "GPT-4o".into(),
            aliases: vec![],
            model_type: ModelType::Chat,
            api_formats: vec![ApiFormat::OpenAI],
            supports_streaming: true,
            supports_tools: true,
            supports_structured_output: false,
            supports_vision: false,
            supports_prefill: false,
            supports_cache: false,
            supports_web_search: false,
            supports_batch: false,
            context_window: 128000,
            cache_enabled: true,
            cache_strategy: "full".into(),
            input_multiplier: 2.5,
            output_multiplier: 10.0,
            cache_write_multiplier: Some(3.75),
            cache_read_multiplier: Some(0.3),
            image_input_multiplier: None,
            audio_input_multiplier: None,
            video_input_multiplier: None,
            image_generation_multiplier: None,
            tts_multiplier: None,
            extra_headers: HashMap::new(),
            enabled: true,
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn extract_key_bearer() {
        let req = Request::builder()
            .header("authorization", "Bearer sk-proxy-test123")
            .body(Body::empty())
            .unwrap();
        assert_eq!(extract_api_key(&req), Some("sk-proxy-test123".into()));
    }

    #[test]
    fn extract_key_x_api_key() {
        let req = Request::builder()
            .header("x-api-key", "sk-proxy-test456")
            .body(Body::empty())
            .unwrap();
        assert_eq!(extract_api_key(&req), Some("sk-proxy-test456".into()));
    }

    #[test]
    fn extract_key_missing() {
        let req = Request::builder().body(Body::empty()).unwrap();
        assert_eq!(extract_api_key(&req), None);
    }

    #[test]
    fn extract_key_bearer_priority() {
        let req = Request::builder()
            .header("authorization", "Bearer sk-bearer")
            .header("x-api-key", "sk-xapi")
            .body(Body::empty())
            .unwrap();
        // Bearer takes priority
        assert_eq!(extract_api_key(&req), Some("sk-bearer".into()));
    }

    #[test]
    fn credit_calculation_text_only() {
        let model = test_model();
        // 1000 input * 2.5/1000 + 500 output * 10.0/1000 = 2.5 + 5.0 = 7.5 → ceil = 8
        let credits = {
            let mut c: f64 = 0.0;
            c += (1000.0 / 1000.0) * model.input_multiplier;
            c += (500.0 / 1000.0) * model.output_multiplier;
            c.ceil() as i64
        };
        assert_eq!(credits, 8);
    }

    #[test]
    fn credit_calculation_with_cache() {
        let model = test_model();
        // input: 100 * 2.5/1000 = 0.25
        // output: 50 * 10.0/1000 = 0.5
        // cache_write: 2000 * 3.75/1000 = 7.5
        // cache_read: 5000 * 0.3/1000 = 1.5
        // total = 9.75 → ceil = 10
        let mut credits: f64 = 0.0;
        credits += (100.0 / 1000.0) * model.input_multiplier;
        credits += (50.0 / 1000.0) * model.output_multiplier;
        credits += (2000.0 / 1000.0) * model.cache_write_multiplier.unwrap();
        credits += (5000.0 / 1000.0) * model.cache_read_multiplier.unwrap();
        assert_eq!(credits.ceil() as i64, 10);
    }

    #[test]
    fn api_key_has_credits_unlimited() {
        let key = ApiKey {
            id: "k1".into(),
            key: "sk-proxy-test".into(),
            name: "test".into(),
            model_group: vec![],
            default_model: String::new(),
            fallback_models: vec![],
            credit_limit: 0, // unlimited
            credit_used: 999999,
            enabled: true,
            created_at: chrono::Utc::now(),
            last_used_at: None,
        };
        assert!(key.has_credits());
    }

    #[test]
    fn api_key_has_credits_within_limit() {
        let key = ApiKey {
            id: "k1".into(),
            key: "sk-proxy-test".into(),
            name: "test".into(),
            model_group: vec![],
            default_model: String::new(),
            fallback_models: vec![],
            credit_limit: 1000,
            credit_used: 500,
            enabled: true,
            created_at: chrono::Utc::now(),
            last_used_at: None,
        };
        assert!(key.has_credits());
    }

    #[test]
    fn api_key_has_credits_exceeded() {
        let key = ApiKey {
            id: "k1".into(),
            key: "sk-proxy-test".into(),
            name: "test".into(),
            model_group: vec![],
            default_model: String::new(),
            fallback_models: vec![],
            credit_limit: 1000,
            credit_used: 1000,
            enabled: true,
            created_at: chrono::Utc::now(),
            last_used_at: None,
        };
        assert!(!key.has_credits());
    }
}
