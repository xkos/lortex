//! /v1/messages — Anthropic Messages API 兼容入口（non-streaming + streaming）

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
use crate::proto::anthropic::*;
use crate::proto::convert::{anthropic_request_to_lortex, lortex_response_to_anthropic};
use crate::state::AppState;

/// 解析模型
async fn resolve_model(
    state: &AppState,
    api_key: &ApiKey,
    model_name: &str,
) -> Result<Model, (StatusCode, Json<AnthropicError>)> {
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
                Json(AnthropicError::new("error", "api_error", "Store error")),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(AnthropicError::not_found(format!(
                    "Model '{}' not found",
                    effective_name
                ))),
            )
        })?;

    if !api_key.model_group.iter().any(|name| model.matches(name)) {
        return Err((
            StatusCode::NOT_FOUND,
            Json(AnthropicError::not_found(format!(
                "Model '{}' not available for this API key",
                effective_name
            ))),
        ));
    }

    if !model.enabled {
        return Err((
            StatusCode::NOT_FOUND,
            Json(AnthropicError::not_found(format!(
                "Model '{}' is disabled",
                effective_name
            ))),
        ));
    }

    Ok(model)
}

/// 构建 provider
async fn build_provider(
    state: &AppState,
    model: &Model,
) -> Result<Arc<dyn Provider>, (StatusCode, Json<AnthropicError>)> {
    let provider_config = state
        .store
        .get_provider(&model.provider_id)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AnthropicError::new("error", "api_error", "Store error")),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AnthropicError::new(
                    "error",
                    "api_error",
                    format!("Provider '{}' not found", model.provider_id),
                )),
            )
        })?;

    if !provider_config.enabled {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(AnthropicError::new(
                "error",
                "api_error",
                format!("Provider '{}' is disabled", model.provider_id),
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

fn map_provider_error(e: ProviderError) -> (StatusCode, Json<AnthropicError>) {
    tracing::error!(error = %e, "Upstream LLM call failed");
    let (status, msg) = match &e {
        ProviderError::RateLimited { .. } => (StatusCode::TOO_MANY_REQUESTS, e.to_string()),
        ProviderError::AuthenticationFailed(_) => {
            (StatusCode::UNAUTHORIZED, "Upstream authentication failed".into())
        }
        _ => (StatusCode::BAD_GATEWAY, e.to_string()),
    };
    (status, Json(AnthropicError::new("error", "api_error", msg)))
}

/// POST /v1/messages — 入口，根据 stream 字段分发
pub async fn messages(
    Extension(state): Extension<AppState>,
    Extension(api_key): Extension<ApiKey>,
    Json(req): Json<MessagesRequest>,
) -> Response {
    if req.stream {
        match messages_stream(state, api_key, req).await {
            Ok(sse) => sse.into_response(),
            Err((status, json)) => (status, json).into_response(),
        }
    } else {
        match messages_blocking(state, api_key, req).await {
            Ok(json) => json.into_response(),
            Err((status, json)) => (status, json).into_response(),
        }
    }
}

/// Non-streaming 路径
async fn messages_blocking(
    state: AppState,
    api_key: ApiKey,
    req: MessagesRequest,
) -> Result<Json<MessagesResponse>, (StatusCode, Json<AnthropicError>)> {
    let model = resolve_model(&state, &api_key, &req.model).await?;
    tracing::info!(
        key_name = %api_key.name,
        requested_model = %req.model,
        resolved_model = %model.id(),
        provider = %model.provider_id,
        endpoint = "/v1/messages",
        stream = false,
        "Routing Anthropic messages request"
    );

    let provider = build_provider(&state, &model).await?;

    let mut lortex_req = anthropic_request_to_lortex(&req);
    lortex_req.model = model.vendor_model_name.clone();

    let start = std::time::Instant::now();
    let lortex_resp = provider.complete(lortex_req).await.map_err(map_provider_error)?;
    let elapsed = start.elapsed();

    if let Some(usage) = &lortex_resp.usage {
        let credits = deduct_credits(
            &state, &api_key, &model,
            usage.prompt_tokens, usage.completion_tokens, 0, 0,
        ).await.unwrap_or(0);

        tracing::info!(
            key_name = %api_key.name,
            model = %model.id(),
            input_tokens = usage.prompt_tokens,
            output_tokens = usage.completion_tokens,
            credits_deducted = credits,
            elapsed_ms = elapsed.as_millis() as u64,
            "Anthropic messages request done"
        );
    }

    let anthropic_resp = lortex_response_to_anthropic(&lortex_resp, &model.id());
    Ok(Json(anthropic_resp))
}

/// Streaming 路径 — 返回 Anthropic SSE 格式
async fn messages_stream(
    state: AppState,
    api_key: ApiKey,
    req: MessagesRequest,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, (StatusCode, Json<AnthropicError>)> {
    let model = resolve_model(&state, &api_key, &req.model).await?;
    tracing::info!(
        key_name = %api_key.name,
        requested_model = %req.model,
        resolved_model = %model.id(),
        provider = %model.provider_id,
        endpoint = "/v1/messages",
        stream = true,
        "Routing Anthropic messages request (streaming)"
    );

    let provider = build_provider(&state, &model).await?;

    let mut lortex_req = anthropic_request_to_lortex(&req);
    lortex_req.model = model.vendor_model_name.clone();

    let model_id = model.id();
    let msg_id = format!("msg_{}", uuid::Uuid::new_v4());

    // Pipe provider stream through channel for 'static lifetime
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

    // Track state across events
    let mut content_block_started = false;
    let mut output_tokens: u32 = 0;

    // Build the SSE stream
    // First: emit message_start
    let msg_id_clone = msg_id.clone();
    let model_id_clone = model_id.clone();

    let init_stream = futures::stream::once(async move {
        let start_event = MessageStartEvent {
            event_type: "message_start".into(),
            message: MessageStartData {
                id: msg_id_clone,
                msg_type: "message".into(),
                role: "assistant".into(),
                content: vec![],
                model: model_id_clone,
                stop_reason: None,
                usage: AnthropicUsage {
                    input_tokens: 0,
                    output_tokens: 0,
                    cache_creation_input_tokens: None,
                    cache_read_input_tokens: None,
                },
            },
        };
        let data = serde_json::to_string(&start_event).unwrap();
        Ok::<_, Infallible>(Event::default().event("message_start").data(data))
    });

    // Then: convert StreamEvents to Anthropic SSE events
    let main_stream = event_stream.map(move |event: Result<StreamEvent, ProviderError>| {
        match event {
            Ok(StreamEvent::ContentDelta { delta }) => {
                let mut events = Vec::new();

                // Emit content_block_start if first text delta
                if !content_block_started {
                    content_block_started = true;
                    let block_start = ContentBlockStartEvent {
                        event_type: "content_block_start".into(),
                        index: 0,
                        content_block: ContentBlock::Text { text: String::new() },
                    };
                    events.push(
                        Event::default()
                            .event("content_block_start")
                            .data(serde_json::to_string(&block_start).unwrap()),
                    );
                }

                let delta_event = ContentBlockDeltaEvent {
                    event_type: "content_block_delta".into(),
                    index: 0,
                    delta: DeltaBlock::TextDelta { text: delta },
                };
                events.push(
                    Event::default()
                        .event("content_block_delta")
                        .data(serde_json::to_string(&delta_event).unwrap()),
                );

                events
            }
            Ok(StreamEvent::ToolCallStart { index, id, name }) => {
                let block_start = ContentBlockStartEvent {
                    event_type: "content_block_start".into(),
                    index,
                    content_block: ContentBlock::ToolUse {
                        id,
                        name,
                        input: serde_json::json!({}),
                    },
                };
                vec![Event::default()
                    .event("content_block_start")
                    .data(serde_json::to_string(&block_start).unwrap())]
            }
            Ok(StreamEvent::ToolCallDelta { index, arguments_delta }) => {
                let delta_event = ContentBlockDeltaEvent {
                    event_type: "content_block_delta".into(),
                    index,
                    delta: DeltaBlock::InputJsonDelta { partial_json: arguments_delta },
                };
                vec![Event::default()
                    .event("content_block_delta")
                    .data(serde_json::to_string(&delta_event).unwrap())]
            }
            Ok(StreamEvent::Done { usage, finish_reason }) => {
                let mut events = Vec::new();

                // Close any open content block
                if content_block_started {
                    let stop = ContentBlockStopEvent {
                        event_type: "content_block_stop".into(),
                        index: 0,
                    };
                    events.push(
                        Event::default()
                            .event("content_block_stop")
                            .data(serde_json::to_string(&stop).unwrap()),
                    );
                }

                // Deduct credits
                if let Some(ref u) = usage {
                    output_tokens = u.completion_tokens;
                    let prompt = u.prompt_tokens;
                    let completion = u.completion_tokens;
                    let state = state.clone();
                    let api_key = api_key.clone();
                    let model = model.clone();
                    let model_id_log = model_id.clone();
                    tokio::spawn(async move {
                        let credits = deduct_credits(
                            &state, &api_key, &model,
                            prompt, completion, 0, 0,
                        ).await.unwrap_or(0);
                        tracing::info!(
                            key_name = %api_key.name,
                            model = %model_id_log,
                            input_tokens = prompt,
                            output_tokens = completion,
                            credits_deducted = credits,
                            "Streaming Anthropic messages done"
                        );
                    });
                }

                let stop_reason = finish_reason.map(|r| match r {
                    lortex_core::provider::FinishReason::Stop => "end_turn",
                    lortex_core::provider::FinishReason::ToolCalls => "tool_use",
                    lortex_core::provider::FinishReason::Length => "max_tokens",
                    lortex_core::provider::FinishReason::ContentFilter => "end_turn",
                }.to_string()).unwrap_or_else(|| "end_turn".into());

                // message_delta
                let msg_delta = MessageDeltaEvent {
                    event_type: "message_delta".into(),
                    delta: MessageDelta { stop_reason },
                    usage: MessageDeltaUsage { output_tokens },
                };
                events.push(
                    Event::default()
                        .event("message_delta")
                        .data(serde_json::to_string(&msg_delta).unwrap()),
                );

                // message_stop
                let msg_stop = MessageStopEvent {
                    event_type: "message_stop".into(),
                };
                events.push(
                    Event::default()
                        .event("message_stop")
                        .data(serde_json::to_string(&msg_stop).unwrap()),
                );

                events
            }
            Err(e) => {
                let err = AnthropicError::new("error", "api_error", e.to_string());
                vec![Event::default()
                    .event("error")
                    .data(serde_json::to_string(&err).unwrap())]
            }
        }
    });

    // Flatten Vec<Event> into individual events
    let flat_stream = main_stream.flat_map(|events| futures::stream::iter(events));

    // Combine init + main, wrap in Ok for Sse
    let full_stream = init_stream.chain(flat_stream.map(Ok));

    Ok(Sse::new(full_stream).keep_alive(KeepAlive::default()))
}
