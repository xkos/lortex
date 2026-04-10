//! /v1/chat/completions — OpenAI 兼容对话补全（non-streaming + streaming）

use std::convert::Infallible;
use std::sync::Arc;

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
use lortex_core::provider::{Provider, StreamEvent};

use crate::middleware::proxy_auth::deduct_credits;
use crate::models::{ApiKey, Model};
use crate::models::provider::Vendor;
use crate::proto::convert::{openai_request_to_lortex, lortex_response_to_openai};
use crate::proto::openai::{
    ChatCompletionChunk, ChatCompletionRequest, ChatMessageDelta, ChunkChoice, ErrorResponse,
    Usage as OaiUsage,
};
use crate::state::AppState;

/// 解析模型：PROXY_MANAGED / 精确 ID / 别名
async fn resolve_model(
    state: &AppState,
    api_key: &ApiKey,
    model_name: &str,
) -> Result<Model, (StatusCode, Json<ErrorResponse>)> {
    let effective_name = if model_name == "PROXY_MANAGED" {
        &api_key.default_model
    } else {
        model_name
    };

    let model = state
        .store
        .find_model(effective_name)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("Store error", "server_error")),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse::not_found(format!(
                    "Model '{}' not found",
                    effective_name
                ))),
            )
        })?;

    if !api_key.model_group.iter().any(|name| model.matches(name)) {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::not_found(format!(
                "Model '{}' not available for this API key",
                effective_name
            ))),
        ));
    }

    if !model.enabled {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse::not_found(format!(
                "Model '{}' is disabled",
                effective_name
            ))),
        ));
    }

    Ok(model)
}

/// 根据 Provider 配置构建 lortex Provider 实例
async fn build_provider(
    state: &AppState,
    model: &Model,
) -> Result<Arc<dyn Provider>, (StatusCode, Json<ErrorResponse>)> {
    let provider_config = state
        .store
        .get_provider(&model.provider_id)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new("Store error", "server_error")),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse::new(
                    format!("Provider '{}' not found", model.provider_id),
                    "server_error",
                )),
            )
        })?;

    if !provider_config.enabled {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse::new(
                format!("Provider '{}' is disabled", model.provider_id),
                "server_error",
            )),
        ));
    }

    let provider: Arc<dyn Provider> = match &provider_config.vendor {
        Vendor::OpenAI | Vendor::DeepSeek | Vendor::Custom(_) => {
            Arc::new(
                lortex_providers::openai::OpenAIProvider::new(&provider_config.api_key)
                    .with_base_url(&provider_config.base_url),
            )
        }
        Vendor::Anthropic => {
            Arc::new(
                lortex_providers::anthropic::AnthropicProvider::new(&provider_config.api_key)
                    .with_base_url(&provider_config.base_url),
            )
        }
    };

    Ok(provider)
}

fn map_provider_error(e: ProviderError) -> (StatusCode, Json<ErrorResponse>) {
    let (status, msg) = match &e {
        ProviderError::RateLimited { .. } => (StatusCode::TOO_MANY_REQUESTS, e.to_string()),
        ProviderError::AuthenticationFailed(_) => {
            (StatusCode::UNAUTHORIZED, "Upstream authentication failed".into())
        }
        _ => (StatusCode::BAD_GATEWAY, e.to_string()),
    };
    (status, Json(ErrorResponse::new(msg, "upstream_error")))
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
    let model = resolve_model(&state, &api_key, &req.model).await?;
    let provider = build_provider(&state, &model).await?;

    let mut lortex_req = openai_request_to_lortex(&req);
    lortex_req.model = model.vendor_model_name.clone();

    let lortex_resp = provider.complete(lortex_req).await.map_err(map_provider_error)?;

    if let Some(usage) = &lortex_resp.usage {
        let _ = deduct_credits(
            &state, &api_key, &model,
            usage.prompt_tokens, usage.completion_tokens, 0, 0,
        ).await;
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
    let model = resolve_model(&state, &api_key, &req.model).await?;
    let provider = build_provider(&state, &model).await?;

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
                    let state = state.clone();
                    let api_key = api_key.clone();
                    let model = model.clone();
                    tokio::spawn(async move {
                        let _ = deduct_credits(
                            &state, &api_key, &model,
                            prompt, completion, 0, 0,
                        ).await;
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
