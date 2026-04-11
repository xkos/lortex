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
use crate::middleware::proxy_auth::deduct_credits;
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
    Json(req): Json<ChatCompletionRequest>,
) -> Response {
    if req.stream {
        match chat_completions_stream(state, api_key, req).await {
            Ok(sse) => sse.into_response(),
            Err((status, json)) => (status, json).into_response(),
        }
    } else {
        match chat_completions_blocking(state, api_key, req).await {
            Ok(json) => json.into_response(),
            Err((status, json)) => (status, json).into_response(),
        }
    }
}

/// Non-streaming 路径
async fn chat_completions_blocking(
    state: AppState,
    api_key: ApiKey,
    req: ChatCompletionRequest,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let model = shared::resolve_model(&state, &api_key, &req.model).await.map_err(to_oai_error)?;
    tracing::info!(
        key_name = %api_key.name,
        requested_model = %req.model,
        resolved_model = %model.id(),
        provider = %model.provider_id,
        stream = false,
        "Routing chat completion"
    );

    let provider = shared::build_provider(&state, &model, &ApiFormat::OpenAI).await.map_err(to_oai_error)?;

    let mut lortex_req = openai_request_to_lortex(&req);
    lortex_req.model = model.vendor_model_name.clone();

    let start = std::time::Instant::now();
    let lortex_resp = provider.complete(lortex_req).await.map_err(|e| to_oai_error(shared::map_provider_error(e)))?;
    let elapsed = start.elapsed();

    if let Some(usage) = &lortex_resp.usage {
        let credits = deduct_credits(
            &state, &api_key, &model,
            usage.prompt_tokens, usage.completion_tokens,
            usage.cache_creation_input_tokens, usage.cache_read_input_tokens,
            "/v1/chat/completions", elapsed.as_millis() as u64,
        ).await.unwrap_or(0);

        tracing::info!(
            key_name = %api_key.name,
            model = %model.id(),
            input_tokens = usage.prompt_tokens,
            output_tokens = usage.completion_tokens,
            total_tokens = usage.total_tokens,
            credits_deducted = credits,
            elapsed_ms = elapsed.as_millis() as u64,
            "Chat completion done"
        );
    }

    let oai_resp = lortex_response_to_openai(&lortex_resp, &model.id());
    Ok(Json(serde_json::to_value(oai_resp).unwrap()))
}

/// Streaming 路径 — 返回 SSE
async fn chat_completions_stream(
    state: AppState,
    api_key: ApiKey,
    req: ChatCompletionRequest,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, (StatusCode, Json<ErrorResponse>)> {
    let model = shared::resolve_model(&state, &api_key, &req.model).await.map_err(to_oai_error)?;
    tracing::info!(
        key_name = %api_key.name,
        requested_model = %req.model,
        resolved_model = %model.id(),
        provider = %model.provider_id,
        stream = true,
        "Routing chat completion (streaming)"
    );

    let provider = shared::build_provider(&state, &model, &ApiFormat::OpenAI).await.map_err(to_oai_error)?;

    let mut lortex_req = openai_request_to_lortex(&req);
    lortex_req.model = model.vendor_model_name.clone();

    let model_id = model.id();
    let completion_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    let created = chrono::Utc::now().timestamp();

    // Provider's complete_stream borrows self, so we pipe through a channel for 'static lifetime
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<StreamEvent, ProviderError>>(256);
    tokio::spawn(async move {
        let mut stream = provider.complete_stream(lortex_req);
        while let Some(event) = stream.next().await {
            if tx.send(event).await.is_err() {
                break;
            }
        }
    });

    let event_stream = tokio_stream::wrappers::ReceiverStream::new(rx);

    let sse_stream = event_stream.map(move |event: Result<StreamEvent, ProviderError>| {
        let chunk = match event {
            Ok(StreamEvent::ContentDelta { delta }) => {
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
                // Deduct credits in background if we have usage
                if let Some(ref u) = usage {
                    let prompt = u.prompt_tokens;
                    let completion = u.completion_tokens;
                    let total = u.total_tokens;
                    let cache_creation = u.cache_creation_input_tokens;
                    let cache_read = u.cache_read_input_tokens;
                    let state = state.clone();
                    let api_key = api_key.clone();
                    let model = model.clone();
                    let model_id_log = model_id.clone();
                    tokio::spawn(async move {
                        let credits = deduct_credits(
                            &state, &api_key, &model,
                            prompt, completion, cache_creation, cache_read,
                            "/v1/chat/completions", 0,
                        ).await.unwrap_or(0);
                        tracing::info!(
                            key_name = %api_key.name,
                            model = %model_id_log,
                            input_tokens = prompt,
                            output_tokens = completion,
                            total_tokens = total,
                            credits_deducted = credits,
                            "Streaming chat completion done"
                        );
                    });
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
