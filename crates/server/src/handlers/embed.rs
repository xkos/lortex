//! /v1/embeddings — OpenAI 兼容 embedding 端点（non-streaming only）

use axum::{
    extract::Extension,
    http::StatusCode,
    response::IntoResponse,
    Json,
};

use crate::handlers::shared::{self, ProxyError};
use crate::layer::helpers::record_model_fields;
use crate::models::model::{ApiFormat, ModelType};
use crate::models::ApiKey;
use crate::proto::openai::{
    EmbeddingObject, EmbeddingRequest, EmbeddingResponse, EmbeddingUsage, ErrorResponse,
};
use crate::state::AppState;

fn to_oai_error(e: ProxyError) -> (StatusCode, Json<ErrorResponse>) {
    (e.status, Json(ErrorResponse::new(e.message, "server_error")))
}

/// POST /v1/embeddings
pub async fn embeddings(
    Extension(state): Extension<AppState>,
    Extension(api_key): Extension<ApiKey>,
    headers: axum::http::HeaderMap,
    Json(req): Json<EmbeddingRequest>,
) -> impl IntoResponse {
    let client_headers = shared::extract_passthrough_headers(&headers);
    match embeddings_inner(state, api_key, client_headers, req).await {
        Ok(json) => json.into_response(),
        Err((status, json)) => (status, json).into_response(),
    }
}

async fn embeddings_inner(
    state: AppState,
    api_key: ApiKey,
    client_headers: std::collections::HashMap<String, String>,
    req: EmbeddingRequest,
) -> Result<Json<EmbeddingResponse>, (StatusCode, Json<ErrorResponse>)> {
    let estimated_chars = serde_json::to_string(&req).map(|s| s.len() as u64).unwrap_or(0);
    let input_texts = req.input.to_vec();

    // 解析候选模型 + fallback
    let models = shared::resolve_models_with_fallback(&state, &api_key, &req.model)
        .await
        .map_err(to_oai_error)?;

    // 校验主模型类型
    if let Some(primary) = models.first() {
        if primary.model_type != ModelType::Embedding {
            return Err(to_oai_error(ProxyError {
                status: StatusCode::BAD_REQUEST,
                message: format!(
                    "Model '{}' is not an embedding model (type: {})",
                    req.model,
                    primary.model_type.as_str()
                ),
            }));
        }
    }

    // 遍历候选模型，找第一个可用的
    let mut last_error = None;
    for model in &models {
        // 熔断器检查
        let available = state
            .circuit_breaker
            .is_available(&model.provider_id)
            .await
            .unwrap_or(true);
        if !available {
            tracing::info!(provider = %model.provider_id, "Skipping circuit-broken provider (embed)");
            continue;
        }

        // 模型级 RPM/TPM 限流
        if model.rpm_limit > 0 {
            if state.rate_limiter.check_model_rpm(&model.id(), model.rpm_limit).is_err() {
                tracing::info!(model = %model.id(), "Skipping RPM-limited model (embed)");
                continue;
            }
        }
        if model.tpm_limit > 0 {
            if state.rate_limiter.check_model_tpm(&model.id(), model.tpm_limit).is_err() {
                tracing::info!(model = %model.id(), "Skipping TPM-limited model (embed)");
                continue;
            }
        }

        // 构建 provider
        let provider = match shared::build_provider_with_headers(
            &state,
            model,
            &ApiFormat::OpenAI,
            &client_headers,
        )
        .await
        {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(model = %model.id(), error = %e.message, "Failed to build provider (embed)");
                last_error = Some(e);
                continue;
            }
        };

        // 构造 embedding 请求
        let embed_req = lortex_core::provider::EmbeddingRequest {
            model: model.vendor_model_name.clone(),
            input: input_texts.clone(),
            encoding_format: req.encoding_format.clone(),
            dimensions: req.dimensions,
        };

        // 调用上游
        match provider.embed(embed_req).await {
            Ok(embed_resp) => {
                // 记录成功
                let _ = state.circuit_breaker.record_success(&model.provider_id).await;
                state.rate_limiter.record_model_request(&model.id());

                // Usage tracking span
                let span = tracing::info_span!(
                    target: "lortex::usage",
                    "proxy_request",
                    api_key_id = %api_key.id,
                    api_key_name = %api_key.name,
                    endpoint = "/v1/embeddings",
                    stream = false,
                    estimated_chars,
                    model_id = tracing::field::Empty,
                    provider_id = tracing::field::Empty,
                    vendor_model_name = tracing::field::Empty,
                    input_multiplier = tracing::field::Empty,
                    output_multiplier = tracing::field::Empty,
                    cache_write_multiplier = tracing::field::Empty,
                    cache_read_multiplier = tracing::field::Empty,
                    input_tokens = tracing::field::Empty,
                    output_tokens = tracing::field::Empty,
                    cache_write_tokens = tracing::field::Empty,
                    cache_read_tokens = tracing::field::Empty,
                );
                record_model_fields(&span, model);
                span.record("input_tokens", embed_resp.usage.prompt_tokens as u64);
                span.record("output_tokens", 0u64);

                // 构造 OpenAI 格式响应
                let response = EmbeddingResponse {
                    object: "list".into(),
                    data: embed_resp
                        .data
                        .into_iter()
                        .map(|d| EmbeddingObject {
                            object: "embedding".into(),
                            index: d.index,
                            embedding: d.embedding,
                        })
                        .collect(),
                    model: model.id(),
                    usage: EmbeddingUsage {
                        prompt_tokens: embed_resp.usage.prompt_tokens,
                        total_tokens: embed_resp.usage.total_tokens,
                    },
                };

                return Ok(Json(response));
            }
            Err(e) => {
                tracing::warn!(
                    model = %model.id(),
                    provider = %model.provider_id,
                    error = %e,
                    "Embedding call failed, checking fallback"
                );
                let _ = state.circuit_breaker.record_failure(&model.provider_id).await;
                last_error = Some(shared::map_provider_error(e));
                continue;
            }
        }
    }

    Err(to_oai_error(last_error.unwrap_or_else(|| {
        ProxyError::unavailable("All embedding models unavailable")
    })))
}
