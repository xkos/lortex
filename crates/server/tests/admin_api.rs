//! Admin API 端到端测试

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

use lortex_server::routes::app_router;
use lortex_server::state::AppState;
use lortex_server::store::SqliteStore;

const ADMIN_KEY: &str = "test-admin-key";

async fn setup() -> axum::Router {
    let store = SqliteStore::new(":memory:").await.unwrap();
    store.migrate().await.unwrap();
    let state = AppState {
        store: Arc::new(store),
    };
    app_router(state, ADMIN_KEY.into())
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

fn unauthed_request(method: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

// --- Auth tests ---

#[tokio::test]
async fn admin_requires_auth() {
    let app = setup().await;
    let resp = app
        .oneshot(unauthed_request("GET", "/admin/api/v1/providers"))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_rejects_wrong_key() {
    let app = setup().await;
    let req = Request::builder()
        .method("GET")
        .uri("/admin/api/v1/providers")
        .header("authorization", "Bearer wrong-key")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
}

// --- Provider CRUD ---

#[tokio::test]
async fn provider_crud() {
    let app = setup().await;

    // Create
    let resp = app
        .clone()
        .oneshot(admin_request(
            "POST",
            "/admin/api/v1/providers",
            Some(r#"{"id":"openai-main","vendor":"openai","display_name":"OpenAI","api_key":"sk-test","base_url":"https://api.openai.com"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // List
    let resp = app
        .clone()
        .oneshot(admin_request("GET", "/admin/api/v1/providers", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let providers: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(providers.as_array().unwrap().len(), 1);

    // Get
    let resp = app
        .clone()
        .oneshot(admin_request("GET", "/admin/api/v1/providers/openai-main", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Update
    let resp = app
        .clone()
        .oneshot(admin_request(
            "PUT",
            "/admin/api/v1/providers/openai-main",
            Some(r#"{"display_name":"OpenAI Updated"}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Delete
    let resp = app
        .clone()
        .oneshot(admin_request("DELETE", "/admin/api/v1/providers/openai-main", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);

    // Verify deleted
    let resp = app
        .oneshot(admin_request("GET", "/admin/api/v1/providers/openai-main", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

// --- Model CRUD ---

#[tokio::test]
async fn model_crud() {
    let app = setup().await;

    // Create
    let resp = app
        .clone()
        .oneshot(admin_request(
            "POST",
            "/admin/api/v1/models",
            Some(r#"{
                "provider_id": "openai",
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

    // List
    let resp = app
        .clone()
        .oneshot(admin_request("GET", "/admin/api/v1/models", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let models: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(models.as_array().unwrap().len(), 1);

    // Get
    let resp = app
        .clone()
        .oneshot(admin_request("GET", "/admin/api/v1/models/openai/gpt-4o", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Delete
    let resp = app
        .clone()
        .oneshot(admin_request("DELETE", "/admin/api/v1/models/openai/gpt-4o", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}

// --- ApiKey CRUD ---

#[tokio::test]
async fn api_key_crud() {
    let app = setup().await;

    // Create
    let resp = app
        .clone()
        .oneshot(admin_request(
            "POST",
            "/admin/api/v1/keys",
            Some(r#"{
                "name": "test-key",
                "model_group": ["openai/gpt-4o"],
                "default_model": "openai/gpt-4o",
                "credit_limit": 100000
            }"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let key_id = created["id"].as_str().unwrap().to_string();
    // Create response should contain full key
    assert!(created["key"].as_str().unwrap().starts_with("sk-proxy-"));

    // List (should show key prefix, not full key)
    let resp = app
        .clone()
        .oneshot(admin_request("GET", "/admin/api/v1/keys", None))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let keys: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(keys.as_array().unwrap().len(), 1);
    assert!(keys[0]["key_prefix"].as_str().unwrap().ends_with("..."));

    // Update
    let resp = app
        .clone()
        .oneshot(admin_request(
            "PUT",
            &format!("/admin/api/v1/keys/{key_id}"),
            Some(r#"{"name":"updated-key","credit_limit":999}"#),
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
    let updated: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(updated["name"], "updated-key");
    assert_eq!(updated["credit_limit"], 999);

    // Reset credits
    let resp = app
        .clone()
        .oneshot(admin_request(
            "POST",
            &format!("/admin/api/v1/keys/{key_id}/reset-credits"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // Delete
    let resp = app
        .clone()
        .oneshot(admin_request(
            "DELETE",
            &format!("/admin/api/v1/keys/{key_id}"),
            None,
        ))
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::NO_CONTENT);
}
