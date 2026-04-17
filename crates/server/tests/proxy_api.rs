//! Proxy API 端到端测试
//!
//! 测试完整链路：鉴权 → 模型解析 → 转发 → credit 扣减
//! 注意：这里不测试真实 LLM 调用，只测试 proxy 层逻辑。
//! 真实 LLM 调用需要 provider 层的 mock，在 003c 中处理。

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use lortex_server::routes::app_router;
use lortex_server::state::AppState;
use lortex_server::store::{ProxyStore, SqliteStore};

const ADMIN_KEY: &str = "test-admin-key";

async fn setup() -> (axum::Router, Arc<SqliteStore>) {
    let store = Arc::new(SqliteStore::new(":memory:").await.unwrap());
    store.migrate().await.unwrap();
    let state = AppState::new(store.clone());
    let app = app_router(state, ADMIN_KEY.into(), false);
    (app, store)
}

fn admin_request(method: &str, uri: &str, body: Option<&str>) -> Request<Body> {
    let builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {ADMIN_KEY}"))
        .header("content-type", "application/json");
    if let Some(b) = body {
        builder.body(Body::from(b.to_string())).unwrap()
    } else {
        builder.body(Body::empty()).unwrap()
    }
}

fn proxy_request(method: &str, uri: &str, api_key: &str, body: Option<&str>) -> Request<Body> {
    let builder = Request::builder()
        .method(method)
        .uri(uri)
        .header("authorization", format!("Bearer {api_key}"))
        .header("content-type", "application/json");
    if let Some(b) = body {
        builder.body(Body::from(b.to_string())).unwrap()
    } else {
        builder.body(Body::empty()).unwrap()
    }
}

/// 通过 admin API 创建测试数据，返回 proxy API key
async fn seed_test_data(app: &axum::Router) -> String {
    // Create provider
    let resp = app
        .clone()
        .oneshot(admin_request(
            "POST",
            "/admin/api/v1/providers",
            Some(r#"{"id":"test-openai","vendor":"openai","display_name":"Test OpenAI","api_key":"sk-fake","base_url":"https://api.openai.com"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Create model
    let resp = app
        .clone()
        .oneshot(admin_request(
            "POST",
            "/admin/api/v1/models",
            Some(r#"{
                "provider_id": "test-openai",
                "vendor_model_name": "gpt-4o",
                "display_name": "GPT-4o",
                "aliases": ["gpt4"],
                "supports_tools": true,
                "input_multiplier": 2.5,
                "output_multiplier": 10.0,
                "context_window": 128000
            }"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // Create API key
    let resp = app
        .clone()
        .oneshot(admin_request(
            "POST",
            "/admin/api/v1/keys",
            Some(r#"{
                "name": "test-key",
                "model_group": ["test-openai/gpt-4o", "gpt4"],
                "default_model": "test-openai/gpt-4o",
                "credit_limit": 100000
            }"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    created["key"].as_str().unwrap().to_string()
}

// --- Auth tests ---

#[tokio::test]
async fn proxy_requires_api_key() {
    let (app, _) = setup().await;
    let resp = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/models")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn proxy_rejects_invalid_key() {
    let (app, _) = setup().await;
    let resp = app
        .oneshot(proxy_request("GET", "/v1/models", "sk-proxy-invalid", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn proxy_rejects_disabled_key() {
    let (app, store) = setup().await;
    let api_key = seed_test_data(&app).await;

    // Disable the key via store
    let mut key = store.get_api_key_by_key(&api_key).await.unwrap().unwrap();
    key.enabled = false;
    store.upsert_api_key(&key).await.unwrap();

    let resp = app
        .oneshot(proxy_request("GET", "/v1/models", &api_key, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn proxy_rejects_exceeded_credits() {
    let (app, store) = setup().await;
    let api_key = seed_test_data(&app).await;

    // Exhaust credits
    let key = store.get_api_key_by_key(&api_key).await.unwrap().unwrap();
    store.add_credits_used(&key.id, 100000).await.unwrap();

    let resp = app
        .oneshot(proxy_request("GET", "/v1/models", &api_key, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
}

// --- /v1/models tests ---

#[tokio::test]
async fn models_returns_key_model_group() {
    let (app, _) = setup().await;
    let api_key = seed_test_data(&app).await;

    let resp = app
        .oneshot(proxy_request("GET", "/v1/models", &api_key, None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let models: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(models["object"], "list");
    let data = models["data"].as_array().unwrap();
    assert_eq!(data.len(), 2);
    assert_eq!(data[0]["id"], "PROXY_MANAGED");
    assert_eq!(data[1]["id"], "test-openai/gpt-4o");
    assert_eq!(data[1]["owned_by"], "test-openai");
}

#[tokio::test]
async fn models_supports_anthropic_auth() {
    let (app, _) = setup().await;
    let api_key = seed_test_data(&app).await;

    // Use x-api-key header instead of Bearer
    let req = Request::builder()
        .method("GET")
        .uri("/v1/models")
        .header("x-api-key", &api_key)
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// --- /v1/chat/completions tests ---
// Note: actual LLM calls will fail (fake API key), but we can test model resolution errors

#[tokio::test]
async fn chat_completions_model_not_found() {
    let (app, _) = setup().await;
    let api_key = seed_test_data(&app).await;

    let resp = app
        .oneshot(proxy_request(
            "POST",
            "/v1/chat/completions",
            &api_key,
            Some(r#"{"model":"nonexistent","messages":[{"role":"user","content":"hi"}]}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn chat_completions_model_not_in_group() {
    let (app, store) = setup().await;
    let api_key = seed_test_data(&app).await;

    // Create another model not in the key's group
    use chrono::Utc;
    use lortex_server::models::model::{ApiFormat, Model, ModelType};
    let model = Model {
        provider_id: "test-openai".into(),
        vendor_model_name: "gpt-4o-mini".into(),
        display_name: "GPT-4o Mini".into(),
        aliases: vec![],
        model_type: ModelType::Chat,
        api_formats: vec![ApiFormat::OpenAI],
        supports_streaming: true,
        supports_tools: false,
        supports_structured_output: false,
        supports_vision: false,
        supports_prefill: false,
        supports_cache: false,
        supports_web_search: false,
        supports_batch: false,
        context_window: 128000,
        cache_enabled: true,
        cache_strategy: "full".into(),
        input_multiplier: 0.15,
        output_multiplier: 0.6,
        cache_write_multiplier: None,
        cache_read_multiplier: None,
        image_input_multiplier: None,
        audio_input_multiplier: None,
        video_input_multiplier: None,
        image_generation_multiplier: None,
        tts_multiplier: None,
        extra_headers: std::collections::HashMap::new(),
        rpm_limit: 0,
        tpm_limit: 0,
        enabled: true,
        created_at: Utc::now(),
    };
    store.upsert_model(&model).await.unwrap();

    let resp = app
        .oneshot(proxy_request(
            "POST",
            "/v1/chat/completions",
            &api_key,
            Some(r#"{"model":"test-openai/gpt-4o-mini","messages":[{"role":"user","content":"hi"}]}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn chat_completions_alias_resolution() {
    let (app, _) = setup().await;
    let api_key = seed_test_data(&app).await;

    // Use alias "gpt4" — model exists and is in group, but will fail at LLM call (fake key)
    // We expect a BAD_GATEWAY (upstream error), not a 404
    let resp = app
        .oneshot(proxy_request(
            "POST",
            "/v1/chat/completions",
            &api_key,
            Some(r#"{"model":"gpt4","messages":[{"role":"user","content":"hi"}]}"#),
        ))
        .await
        .unwrap();
    // Should NOT be 404 — alias resolved successfully
    assert_ne!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn chat_completions_proxy_managed() {
    let (app, _) = setup().await;
    let api_key = seed_test_data(&app).await;

    let resp = app
        .oneshot(proxy_request(
            "POST",
            "/v1/chat/completions",
            &api_key,
            Some(r#"{"model":"PROXY_MANAGED","messages":[{"role":"user","content":"hi"}]}"#),
        ))
        .await
        .unwrap();
    // Should resolve to default_model, not 404
    assert_ne!(resp.status(), StatusCode::NOT_FOUND);
}

// --- /v1/messages (Anthropic) tests ---

#[tokio::test]
async fn messages_requires_auth() {
    let (app, _) = setup().await;
    let req = Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"model":"test","max_tokens":100,"messages":[{"role":"user","content":"hi"}]}"#,
        ))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn messages_model_not_found() {
    let (app, _) = setup().await;
    let api_key = seed_test_data(&app).await;

    let resp = app
        .oneshot(proxy_request(
            "POST",
            "/v1/messages",
            &api_key,
            Some(r#"{"model":"nonexistent","max_tokens":100,"messages":[{"role":"user","content":"hi"}]}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn messages_resolves_model() {
    let (app, _) = setup().await;
    let api_key = seed_test_data(&app).await;

    // Use a valid model — will fail at LLM call (fake key) but should resolve model OK
    let resp = app
        .oneshot(proxy_request(
            "POST",
            "/v1/messages",
            &api_key,
            Some(r#"{"model":"test-openai/gpt-4o","max_tokens":100,"messages":[{"role":"user","content":"hi"}]}"#),
        ))
        .await
        .unwrap();
    // Should NOT be 404 — model resolved
    assert_ne!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn messages_with_x_api_key_header() {
    let (app, _) = setup().await;
    let api_key = seed_test_data(&app).await;

    // x-api-key auth should work — model resolves, upstream will fail (fake key)
    // but we should NOT get a proxy-level "Missing API key" or "Invalid API key" error
    let req = Request::builder()
        .method("POST")
        .uri("/v1/messages")
        .header("x-api-key", api_key.as_str())
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"model":"test-openai/gpt-4o","max_tokens":100,"messages":[{"role":"user","content":"hi"}]}"#,
        ))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    // Model resolved (not 404), auth passed (upstream error, not proxy auth error)
    assert_ne!(resp.status(), StatusCode::NOT_FOUND);
    // Verify it's an upstream error, not a proxy auth error
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let body_str = String::from_utf8_lossy(&body);
    assert!(!body_str.contains("Missing API key"));
    assert!(!body_str.contains("Invalid API key"));
}

#[tokio::test]
async fn messages_proxy_managed() {
    let (app, _) = setup().await;
    let api_key = seed_test_data(&app).await;

    let resp = app
        .oneshot(proxy_request(
            "POST",
            "/v1/messages",
            &api_key,
            Some(r#"{"model":"PROXY_MANAGED","max_tokens":100,"messages":[{"role":"user","content":"hi"}]}"#),
        ))
        .await
        .unwrap();
    assert_ne!(resp.status(), StatusCode::NOT_FOUND);
}
