//! Proxy API Key 鉴权中间件

use std::time::Duration;

use axum::{
    extract::Request,
    http::{HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};

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

    // RPM 检查
    if let Err(reset_after) = state.rate_limiter.check_rpm(&api_key.id, api_key.rpm_limit) {
        tracing::warn!(
            key_id = %api_key.id,
            key_name = %api_key.name,
            rpm_limit = api_key.rpm_limit,
            reset_after_s = reset_after.as_secs(),
            "RPM limit exceeded"
        );
        return Err(rate_limit_response(
            "RPM limit exceeded",
            reset_after,
            api_key.rpm_limit,
        ));
    }

    // TPM 检查
    if let Err(reset_after) = state.rate_limiter.check_tpm(&api_key.id, api_key.tpm_limit) {
        tracing::warn!(
            key_id = %api_key.id,
            key_name = %api_key.name,
            tpm_limit = api_key.tpm_limit,
            reset_after_s = reset_after.as_secs(),
            "TPM limit exceeded"
        );
        return Err(rate_limit_response(
            "TPM limit exceeded",
            reset_after,
            api_key.tpm_limit,
        ));
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

/// 构造 429 响应，带 x-ratelimit-* 头和 Retry-After
fn rate_limit_response(message: &str, reset_after: Duration, limit: u32) -> Response {
    let reset_secs = reset_after.as_secs_f64().ceil() as u64;
    let mut response = (
        StatusCode::TOO_MANY_REQUESTS,
        Json(ErrorResponse::rate_limit(message)),
    )
        .into_response();

    let headers = response.headers_mut();
    if let Ok(v) = HeaderValue::from_str(&limit.to_string()) {
        headers.insert("x-ratelimit-limit-requests", v);
    }
    if let Ok(v) = HeaderValue::from_str(&format!("{reset_secs}s")) {
        headers.insert("x-ratelimit-reset-requests", v);
    }
    if let Ok(v) = HeaderValue::from_str(&reset_secs.to_string()) {
        headers.insert("retry-after", v);
    }
    response
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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;

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

}
