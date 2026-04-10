//! Admin 鉴权中间件

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::Response,
};

/// 从请求中提取 admin key 并验证
pub async fn admin_auth(
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let admin_key = request
        .extensions()
        .get::<AdminKey>()
        .map(|k| k.0.clone())
        .unwrap_or_default();

    let provided = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");

    if admin_key.is_empty() || provided != admin_key {
        return Err(StatusCode::UNAUTHORIZED);
    }

    Ok(next.run(request).await)
}

/// Admin key 注入到 request extensions
#[derive(Clone)]
pub struct AdminKey(pub String);
