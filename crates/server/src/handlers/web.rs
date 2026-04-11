//! Admin Web 静态文件服务

use axum::{
    extract::Path,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use rust_embed::Embed;

/// 嵌入前端构建产物
/// 构建前端后，产物放在 crates/server/admin-web/dist/ 目录
#[derive(Embed)]
#[folder = "admin-web/dist/"]
#[prefix = ""]
struct AdminWebAssets;

/// GET /admin/web/ — 返回 index.html
pub async fn index() -> Response {
    match serve_file("index.html") {
        Some(resp) => resp,
        None => (StatusCode::NOT_FOUND, "Admin web not available").into_response(),
    }
}

/// GET /admin/web/*path — 返回静态文件，找不到则 fallback 到 index.html（SPA 路由）
pub async fn static_file(Path(path): Path<String>) -> Response {
    // 先尝试精确匹配
    if let Some(resp) = serve_file(&path) {
        return resp;
    }
    // SPA fallback: 非文件路径返回 index.html
    match serve_file("index.html") {
        Some(resp) => resp,
        None => (StatusCode::NOT_FOUND, "Admin web not available").into_response(),
    }
}

fn serve_file(path: &str) -> Option<Response> {
    let asset = AdminWebAssets::get(path)?;
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    Some(
        (
            [(header::CONTENT_TYPE, mime.as_ref())],
            asset.data.to_vec(),
        )
            .into_response(),
    )
}
