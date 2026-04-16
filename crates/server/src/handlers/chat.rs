//! /v1/chat/completions — OpenAI 兼容对话补全（non-streaming + streaming）

use std::convert::Infallible;

use axum::{
    extract::Extension,
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use futures::StreamExt;

use lortex_core::error::ProviderError;
use lortex_core::provider::StreamEvent;

use crate::handlers::shared::{self, ProxyError};
use crate::layer::helpers::{record_model_fields, record_usage_fields};
use crate::models::model::ApiFormat;
use crate::models::ApiKey;
use crate::proto::convert::{openai_request_to_lortex, lortex_response_to_openai};
use crate::proto::openai::{
    ChatCompletionChunk, ChatCompletionRequest, ChatMessageDelta, ChunkChoice, ErrorResponse,
    Usage as OaiUsage,
};
use crate::state::AppState;

fn to_oai_error(e: ProxyError) -> (StatusCode, Json<ErrorResponse>) {
    (e.status, Json(ErrorResponse::new(e.message, "server_error")))
}

/// POST /v1/chat/completions — 入口，根据 stream 字段分发
pub async fn chat_completions(
    Extension(state): Extension<AppState>,
    Extension(api_key): Extension<ApiKey>,
    headers: axum::http::HeaderMap,
    Json(req): Json<ChatCompletionRequest>,
) -> Response {
    let client_headers = shared::extract_passthrough_headers(&headers);
    if req.stream {
        match chat_completions_stream(state, api_key, client_headers, req).await {
            Ok(sse) => sse.into_response(),
            Err((status, json)) => (status, json).into_response(),
        }
    } else {
        match chat_completions_blocking(state, api_key, client_headers, req).await {
            Ok(json) => json.into_response(),
            Err((status, json)) => (status, json).into_response(),
        }
    }
}

/// Non-streaming 路径
async fn chat_completions_blocking(
    state: AppState,
    api_key: ApiKey,
    client_headers: std::collections::HashMap<String, String>,
    req: ChatCompletionRequest,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let estimated_chars = serde_json::to_string(&req).map(|s| s.len() as u64).unwrap_or(0);

    let span = tracing::info_span!(
        target: "lortex::usage",
        "proxy_request",
        api_key_id = %api_key.id,
        api_key_name = %api_key.name,
        endpoint = "/v1/chat/completions",
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

    let (lortex_resp, model) = shared::complete_with_fallback(
        &state,
        &api_key,
        &req.model,
        &ApiFormat::OpenAI,
        &client_headers,
        |model| {
            let mut lortex_req = openai_request_to_lortex(&req);
            lortex_req.model = model.vendor_model_name.clone();
            lortex_req
        },
    )
    .await
    .map_err(to_oai_error)?;

    record_model_fields(&span, &model);

    if let Some(usage) = &lortex_resp.usage {
        record_usage_fields(&span, usage);
    }

    let oai_resp = lortex_response_to_openai(&lortex_resp, &model.id());
    Ok(Json(serde_json::to_value(oai_resp).unwrap()))
}

/// Streaming 路径 — 返回 SSE
async fn chat_completions_stream(
    state: AppState,
    api_key: ApiKey,
    client_headers: std::collections::HashMap<String, String>,
    req: ChatCompletionRequest,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, (StatusCode, Json<ErrorResponse>)> {
    let start = std::time::Instant::now();
    let estimated_chars = serde_json::to_string(&req).map(|s| s.len() as u64).unwrap_or(0);

    // 解析主模型 + fallback，选第一个可用的
    let models = shared::resolve_models_with_fallback(&state, &api_key, &req.model)
        .await
        .map_err(to_oai_error)?;

    let mut model = None;
    let mut provider = None;
    for m in &models {
        let available = state.circuit_breaker.is_available(&m.provider_id).await.unwrap_or(true);
        if !available {
            tracing::info!(provider = %m.provider_id, "Skipping circuit-broken provider (stream)");
            continue;
        }
        // 检查模型级 RPM/TPM 限流
        if m.rpm_limit > 0 {
            if state.rate_limiter.check_model_rpm(&m.id(), m.rpm_limit).is_err() {
                tracing::info!(model = %m.id(), "Skipping RPM-limited model (stream)");
                continue;
            }
        }
        if m.tpm_limit > 0 {
            if state.rate_limiter.check_model_tpm(&m.id(), m.tpm_limit).is_err() {
                tracing::info!(model = %m.id(), "Skipping TPM-limited model (stream)");
                continue;
            }
        }
        match shared::build_provider_with_headers(&state, m, &ApiFormat::OpenAI, &client_headers).await {
            Ok(p) => {
                model = Some(m.clone());
                provider = Some(p);
                // 记录模型级 RPM
                state.rate_limiter.record_model_request(&m.id());
                break;
            }
            Err(e) => {
                tracing::warn!(model = %m.id(), error = %e.message, "Failed to build provider (stream)");
            }
        }
    }
    let model = model.ok_or_else(|| to_oai_error(shared::ProxyError::unavailable("All models unavailable")))?;
    let provider = provider.unwrap();

    let span = tracing::info_span!(
        target: "lortex::usage",
        "proxy_request",
        api_key_id = %api_key.id,
        api_key_name = %api_key.name,
        endpoint = "/v1/chat/completions",
        stream = true,
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
        ttft_ms = tracing::field::Empty,
    );
    record_model_fields(&span, &model);

    let model_id = model.id();
    let completion_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    let created = chrono::Utc::now().timestamp();

    // Provider's complete_stream borrows self, so we pipe through a channel for 'static lifetime
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<StreamEvent, ProviderError>>(256);
    let mut lortex_req = openai_request_to_lortex(&req);
    lortex_req.model = model.vendor_model_name.clone();

    tokio::spawn(async move {
        let mut stream = provider.complete_stream(lortex_req);
        while let Some(event) = stream.next().await {
            if tx.send(event).await.is_err() {
                break;
            }
        }
    });

    let event_stream = tokio_stream::wrappers::ReceiverStream::new(rx);

    let mut ttft_recorded = false;

    // span 移入闭包，stream 结束时 span drop → UsageLayer::on_close 触发
    let sse_stream = event_stream.map(move |event: Result<StreamEvent, ProviderError>| {
        let chunk = match event {
            Ok(StreamEvent::ContentDelta { delta }) => {
                if !ttft_recorded {
                    ttft_recorded = true;
                    span.record("ttft_ms", start.elapsed().as_millis() as u64);
                }
                let chunk = ChatCompletionChunk {
                    id: completion_id.clone(),
                    object: "chat.completion.chunk".into(),
                    created,
                    model: model_id.clone(),
                    choices: vec![ChunkChoice {
                        index: 0,
                        delta: ChatMessageDelta {
                            role: None,
                            content: Some(delta),
                            tool_calls: None,
                        },
                        finish_reason: None,
                    }],
                    usage: None,
                };
                serde_json::to_string(&chunk).unwrap()
            }
            Ok(StreamEvent::Done { usage, finish_reason }) => {
                if let Some(ref u) = usage {
                    record_usage_fields(&span, u);
                }

                let oai_usage = usage.map(|u| OaiUsage {
                    prompt_tokens: u.prompt_tokens,
                    completion_tokens: u.completion_tokens,
                    total_tokens: u.total_tokens,
                    prompt_tokens_details: None,
                });

                let fr = finish_reason.map(|r| match r {
                    lortex_core::provider::FinishReason::Stop => "stop".to_string(),
                    lortex_core::provider::FinishReason::ToolCalls => "tool_calls".to_string(),
                    lortex_core::provider::FinishReason::Length => "length".to_string(),
                    lortex_core::provider::FinishReason::ContentFilter => "content_filter".to_string(),
                });

                let chunk = ChatCompletionChunk {
                    id: completion_id.clone(),
                    object: "chat.completion.chunk".into(),
                    created,
                    model: model_id.clone(),
                    choices: vec![ChunkChoice {
                        index: 0,
                        delta: ChatMessageDelta {
                            role: None,
                            content: None,
                            tool_calls: None,
                        },
                        finish_reason: fr,
                    }],
                    usage: oai_usage,
                };
                serde_json::to_string(&chunk).unwrap()
            }
            Ok(StreamEvent::ToolCallStart { index, id, name }) => {
                let chunk = ChatCompletionChunk {
                    id: completion_id.clone(),
                    object: "chat.completion.chunk".into(),
                    created,
                    model: model_id.clone(),
                    choices: vec![ChunkChoice {
                        index: 0,
                        delta: ChatMessageDelta {
                            role: None,
                            content: None,
                            tool_calls: Some(vec![crate::proto::openai::ToolCallDelta {
                                index: index as u32,
                                id: Some(id),
                                call_type: Some("function".into()),
                                function: Some(crate::proto::openai::FunctionCallDelta {
                                    name: Some(name),
                                    arguments: None,
                                }),
                            }]),
                        },
                        finish_reason: None,
                    }],
                    usage: None,
                };
                serde_json::to_string(&chunk).unwrap()
            }
            Ok(StreamEvent::ToolCallDelta { index, arguments_delta }) => {
                let chunk = ChatCompletionChunk {
                    id: completion_id.clone(),
                    object: "chat.completion.chunk".into(),
                    created,
                    model: model_id.clone(),
                    choices: vec![ChunkChoice {
                        index: 0,
                        delta: ChatMessageDelta {
                            role: None,
                            content: None,
                            tool_calls: Some(vec![crate::proto::openai::ToolCallDelta {
                                index: index as u32,
                                id: None,
                                call_type: None,
                                function: Some(crate::proto::openai::FunctionCallDelta {
                                    name: None,
                                    arguments: Some(arguments_delta),
                                }),
                            }]),
                        },
                        finish_reason: None,
                    }],
                    usage: None,
                };
                serde_json::to_string(&chunk).unwrap()
            }
            Err(e) => {
                // Send error as a data event, then the stream will end
                serde_json::json!({"error": {"message": e.to_string(), "type": "upstream_error"}}).to_string()
            }
        };

        Ok::<_, Infallible>(Event::default().data(chunk))
    });

    // Append [DONE] sentinel after the stream ends
    let done_stream = futures::stream::once(async {
        Ok::<_, Infallible>(Event::default().data("[DONE]"))
    });

    let full_stream = sse_stream.chain(done_stream);

    Ok(Sse::new(full_stream).keep_alive(KeepAlive::default()))
}
