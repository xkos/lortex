//! /v1/messages — Anthropic Messages API 兼容入口（non-streaming + streaming）

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
use crate::proto::anthropic::*;
use crate::proto::convert::{anthropic_request_to_lortex, lortex_response_to_anthropic};
use crate::state::AppState;

fn to_anthropic_error(e: ProxyError) -> (StatusCode, Json<AnthropicError>) {
    (e.status, Json(AnthropicError::new("error", "api_error", e.message)))
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
    let start = std::time::Instant::now();

    let (lortex_resp, model) = shared::complete_with_fallback(
        &state,
        &api_key,
        &req.model,
        &ApiFormat::Anthropic,
        |model| {
            let mut lortex_req = anthropic_request_to_lortex(&req);
            lortex_req.model = model.vendor_model_name.clone();
            lortex_req
        },
    )
    .await
    .map_err(to_anthropic_error)?;

    let elapsed = start.elapsed();

    if let Some(usage) = &lortex_resp.usage {
        let credits = deduct_credits(
            &state, &api_key, &model,
            usage.prompt_tokens, usage.completion_tokens,
            usage.cache_creation_input_tokens, usage.cache_read_input_tokens,
            "/v1/messages", elapsed.as_millis() as u64,
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
    // 解析主模型 + fallback，选第一个可用的
    let models = shared::resolve_models_with_fallback(&state, &api_key, &req.model)
        .await
        .map_err(to_anthropic_error)?;

    let mut model = None;
    let mut provider = None;
    for m in &models {
        let available = state.circuit_breaker.is_available(&m.provider_id).await.unwrap_or(true);
        if !available {
            tracing::info!(provider = %m.provider_id, "Skipping circuit-broken provider (stream)");
            continue;
        }
        match shared::build_provider(&state, m, &ApiFormat::Anthropic).await {
            Ok(p) => {
                model = Some(m.clone());
                provider = Some(p);
                break;
            }
            Err(e) => {
                tracing::warn!(model = %m.id(), error = %e.message, "Failed to build provider (stream)");
            }
        }
    }
    let model = model.ok_or_else(|| to_anthropic_error(shared::ProxyError::unavailable("All models unavailable")))?;
    let provider = provider.unwrap();

    tracing::info!(
        key_name = %api_key.name,
        requested_model = %req.model,
        resolved_model = %model.id(),
        provider = %model.provider_id,
        endpoint = "/v1/messages",
        stream = true,
        "Routing Anthropic messages request (streaming)"
    );

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
    let mut next_block_index: usize = 0;  // Next Anthropic content block index
    let mut text_block_open = false;
    let mut current_tool_indices: std::collections::HashMap<usize, usize> = std::collections::HashMap::new(); // OpenAI tool index → Anthropic block index
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
                if !text_block_open {
                    text_block_open = true;
                    let block_start = ContentBlockStartEvent {
                        event_type: "content_block_start".into(),
                        index: next_block_index,
                        content_block: ContentBlock::Text { text: String::new() },
                    };
                    events.push(
                        Event::default()
                            .event("content_block_start")
                            .data(serde_json::to_string(&block_start).unwrap()),
                    );
                    // Don't increment next_block_index yet — will do on close
                }

                let delta_event = ContentBlockDeltaEvent {
                    event_type: "content_block_delta".into(),
                    index: if text_block_open { next_block_index } else { 0 },
                    delta: DeltaBlock::TextDelta { text: delta },
                };
                events.push(
                    Event::default()
                        .event("content_block_delta")
                        .data(serde_json::to_string(&delta_event).unwrap()),
                );

                events
            }
            Ok(StreamEvent::ToolCallStart { index: oai_index, id, name }) => {
                let mut events = Vec::new();

                // Close text block if open
                if text_block_open {
                    text_block_open = false;
                    let stop = ContentBlockStopEvent {
                        event_type: "content_block_stop".into(),
                        index: next_block_index,
                    };
                    events.push(
                        Event::default()
                            .event("content_block_stop")
                            .data(serde_json::to_string(&stop).unwrap()),
                    );
                    next_block_index += 1;
                }

                // Close previous tool block if any (different index)
                // OpenAI sends ToolCallStart for each new tool
                // We need to close the previous tool's block
                if let Some(&prev_block_idx) = current_tool_indices.values().last() {
                    if !current_tool_indices.contains_key(&oai_index) {
                        let stop = ContentBlockStopEvent {
                            event_type: "content_block_stop".into(),
                            index: prev_block_idx,
                        };
                        events.push(
                            Event::default()
                                .event("content_block_stop")
                                .data(serde_json::to_string(&stop).unwrap()),
                        );
                        next_block_index += 1;
                    }
                }

                // Map OpenAI tool index to Anthropic block index
                let block_idx = next_block_index;
                current_tool_indices.insert(oai_index, block_idx);

                let block_start = ContentBlockStartEvent {
                    event_type: "content_block_start".into(),
                    index: block_idx,
                    content_block: ContentBlock::ToolUse {
                        id,
                        name,
                        input: serde_json::json!({}),
                    },
                };
                events.push(
                    Event::default()
                        .event("content_block_start")
                        .data(serde_json::to_string(&block_start).unwrap()),
                );

                events
            }
            Ok(StreamEvent::ToolCallDelta { index: oai_index, arguments_delta }) => {
                let block_idx = current_tool_indices.get(&oai_index).copied().unwrap_or(oai_index);
                let delta_event = ContentBlockDeltaEvent {
                    event_type: "content_block_delta".into(),
                    index: block_idx,
                    delta: DeltaBlock::InputJsonDelta { partial_json: arguments_delta },
                };
                vec![Event::default()
                    .event("content_block_delta")
                    .data(serde_json::to_string(&delta_event).unwrap())]
            }
            Ok(StreamEvent::Done { usage, finish_reason }) => {
                let mut events = Vec::new();

                // Close text block if still open
                if text_block_open {
                    let stop = ContentBlockStopEvent {
                        event_type: "content_block_stop".into(),
                        index: next_block_index,
                    };
                    events.push(
                        Event::default()
                            .event("content_block_stop")
                            .data(serde_json::to_string(&stop).unwrap()),
                    );
                }

                // Close last tool block if any
                if let Some(&last_block_idx) = current_tool_indices.values().max() {
                    let stop = ContentBlockStopEvent {
                        event_type: "content_block_stop".into(),
                        index: last_block_idx,
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
                            "/v1/messages", 0,
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
