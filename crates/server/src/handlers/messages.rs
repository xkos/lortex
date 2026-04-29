//! /v1/messages — Anthropic Messages API 兼容入口（non-streaming + streaming）
//! 同格式时走 passthrough（原样转发），异格式走 Lortex 转换

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
use bytes::Bytes;
use futures::StreamExt;
use serde_json::Value;

use lortex_core::error::ProviderError;
use lortex_core::provider::StreamEvent;

use crate::handlers::passthrough::{
    self, extract_usage_anthropic, forward_blocking, forward_stream, prepare_body,
    PassthroughConfig,
};
use crate::handlers::provider_builder::merge_headers;
use crate::handlers::shared::{self, ProxyError};
use crate::layer::helpers::{record_model_fields, record_usage_fields};
use crate::models::model::ApiFormat;
use crate::models::ApiKey;
use crate::proto::anthropic::*;
use crate::proto::convert::{anthropic_request_to_lortex, lortex_response_to_anthropic};
use crate::state::AppState;

fn to_anthropic_error(e: ProxyError) -> (StatusCode, Json<AnthropicError>) {
    (e.status, Json(AnthropicError::new("error", "api_error", e.message)))
}

fn anthropic_error_response(e: ProxyError) -> Response {
    tracing::warn!(
        status = %e.status,
        endpoint = "/v1/messages",
        "{}", e.message
    );
    let (status, json) = to_anthropic_error(e);
    (status, json).into_response()
}

/// POST /v1/messages — 入口：先解析 model+stream，判断 passthrough 后分发
pub async fn messages(
    Extension(state): Extension<AppState>,
    Extension(api_key): Extension<ApiKey>,
    headers: axum::http::HeaderMap,
    body: Bytes,
) -> Response {
    let client_headers = shared::extract_passthrough_headers(&headers);

    let body_value: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            return anthropic_error_response(ProxyError {
                status: StatusCode::BAD_REQUEST,
                message: format!("Invalid JSON: {e}"),
            });
        }
    };

    let model_name = body_value
        .get("model")
        .and_then(|v| v.as_str())
        .unwrap_or("PROXY_MANAGED");
    let is_stream = body_value
        .get("stream")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let target = match shared::resolve_target(&state, &api_key, model_name, &ApiFormat::Anthropic)
        .await
    {
        Ok(t) => t,
        Err(e) => return anthropic_error_response(e),
    };

    if target.passthrough {
        let base = target.provider_config.base_url.trim_end_matches('/');
        let config = PassthroughConfig {
            upstream_url: format!("{}/v1/messages", base),
            api_key: target.provider_config.api_key.clone(),
            format: ApiFormat::Anthropic,
            auth_scheme: target.provider_config.auth_scheme,
            vendor_model_name: target.model.vendor_model_name.clone(),
            extra_headers: merge_headers(&target.model.extra_headers, &client_headers),
        };
        let prepared = match prepare_body(&body, &config) {
            Ok(b) => b,
            Err(e) => return anthropic_error_response(e),
        };

        let upstream_url = config.upstream_url.clone();
        if is_stream {
            match passthrough_stream_anthropic(&state, &api_key, &target.model, config, prepared)
                .await
            {
                Ok(resp) => resp,
                Err(e) => {
                    tracing::warn!(
                        upstream_url = %upstream_url,
                        provider = %target.model.provider_id,
                        "Passthrough stream failed"
                    );
                    anthropic_error_response(e)
                }
            }
        } else {
            match passthrough_blocking_anthropic(&state, &api_key, &target.model, config, prepared)
                .await
            {
                Ok(resp) => resp,
                Err(e) => anthropic_error_response(e),
            }
        }
    } else {
        let req: MessagesRequest = match serde_json::from_value(body_value) {
            Ok(r) => r,
            Err(e) => {
                return anthropic_error_response(ProxyError {
                    status: StatusCode::UNPROCESSABLE_ENTITY,
                    message: format!("Failed to deserialize the JSON body into the target type: {e}"),
                });
            }
        };
        if req.stream {
            match messages_stream(state, api_key, client_headers, req).await {
                Ok(sse) => sse.into_response(),
                Err((status, json)) => (status, json).into_response(),
            }
        } else {
            match messages_blocking(state, api_key, client_headers, req).await {
                Ok(json) => json.into_response(),
                Err((status, json)) => (status, json).into_response(),
            }
        }
    }
}

/// Non-streaming 路径
async fn messages_blocking(
    state: AppState,
    api_key: ApiKey,
    client_headers: std::collections::HashMap<String, String>,
    req: MessagesRequest,
) -> Result<Json<MessagesResponse>, (StatusCode, Json<AnthropicError>)> {
    let estimated_chars = serde_json::to_string(&req).map(|s| s.len() as u64).unwrap_or(0);

    let span = tracing::info_span!(
        target: "lortex::usage",
        "proxy_request",
        api_key_id = %api_key.id,
        api_key_name = %api_key.name,
        endpoint = "/v1/messages",
        stream = false,
        estimated_chars,
        model_id = tracing::field::Empty,
        provider_id = tracing::field::Empty,
        vendor_model_name = tracing::field::Empty,
        input_tokens = tracing::field::Empty,
        output_tokens = tracing::field::Empty,
        cache_write_tokens = tracing::field::Empty,
        cache_read_tokens = tracing::field::Empty,
    );

    let (lortex_resp, model) = shared::complete_with_fallback(
        &state,
        &api_key,
        &req.model,
        &ApiFormat::Anthropic,
        &client_headers,
        |model| {
            let mut lortex_req = anthropic_request_to_lortex(&req);
            lortex_req.model = model.vendor_model_name.clone();
            lortex_req
        },
    )
    .await
    .map_err(to_anthropic_error)?;

    record_model_fields(&span, &model);

    if let Some(usage) = &lortex_resp.usage {
        record_usage_fields(&span, usage);
    }

    let anthropic_resp = lortex_response_to_anthropic(&lortex_resp, &model.id());
    Ok(Json(anthropic_resp))
}

/// Streaming 路径 — 返回 Anthropic SSE 格式
async fn messages_stream(
    state: AppState,
    api_key: ApiKey,
    client_headers: std::collections::HashMap<String, String>,
    req: MessagesRequest,
) -> Result<Sse<impl futures::Stream<Item = Result<Event, Infallible>>>, (StatusCode, Json<AnthropicError>)> {
    let start = std::time::Instant::now();
    let estimated_chars = serde_json::to_string(&req).map(|s| s.len() as u64).unwrap_or(0);

    // 解析主模型 + fallback，选第一个可用的
    let models = shared::resolve_models_with_fallback(&state, &api_key, &req.model)
        .await
        .map_err(to_anthropic_error)?;

    let mut model = None;
    let mut provider = None;
    for m in &models {
        let available = state.circuit_breaker.is_available(&m.id()).await.unwrap_or(true);
        if !available {
            tracing::info!(model = %m.id(), "Skipping circuit-broken model (stream)");
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
        match shared::build_provider_with_headers(&state, m, &ApiFormat::Anthropic, &client_headers).await {
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
    let model = model.ok_or_else(|| to_anthropic_error(shared::ProxyError::unavailable("All models unavailable")))?;
    let provider = provider.unwrap();

    let span = tracing::info_span!(
        target: "lortex::usage",
        "proxy_request",
        api_key_id = %api_key.id,
        api_key_name = %api_key.name,
        endpoint = "/v1/messages",
        stream = true,
        estimated_chars,
        model_id = tracing::field::Empty,
        provider_id = tracing::field::Empty,
        vendor_model_name = tracing::field::Empty,
        input_tokens = tracing::field::Empty,
        output_tokens = tracing::field::Empty,
        cache_write_tokens = tracing::field::Empty,
        cache_read_tokens = tracing::field::Empty,
        ttft_ms = tracing::field::Empty,
    );
    record_model_fields(&span, &model);

    let model_id = model.id();
    let msg_id = format!("msg_{}", uuid::Uuid::new_v4());

    let mut lortex_req = anthropic_request_to_lortex(&req);
    lortex_req.model = model.vendor_model_name.clone();

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
    let mut next_block_index: usize = 0;
    let mut text_block_open = false;
    let mut current_tool_indices: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    let mut output_tokens: u32 = 0;
    let mut message_start_sent = false;

    // span 移入闭包，stream 结束时 span drop → UsageLayer::on_close 触发
    let main_stream = event_stream.map(move |event: Result<StreamEvent, ProviderError>| {
        let mut events = Vec::new();

        // Emit message_start on first upstream event (reflects real TTFB)
        if !message_start_sent {
            message_start_sent = true;
            span.record("ttft_ms", start.elapsed().as_millis() as u64);
            let start_event = MessageStartEvent {
                event_type: "message_start".into(),
                message: MessageStartData {
                    id: msg_id.clone(),
                    msg_type: "message".into(),
                    role: "assistant".into(),
                    content: vec![],
                    model: model_id.clone(),
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
            events.push(Event::default().event("message_start").data(data));
        }

        match event {
            Ok(StreamEvent::ContentDelta { delta }) => {
                if !text_block_open {
                    text_block_open = true;
                    let block_start = ContentBlockStartEvent {
                        event_type: "content_block_start".into(),
                        index: next_block_index,
                        content_block: ContentBlock::Text { text: String::new(), cache_control: None },
                    };
                    events.push(
                        Event::default()
                            .event("content_block_start")
                            .data(serde_json::to_string(&block_start).unwrap()),
                    );
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
            }
            Ok(StreamEvent::ToolCallStart { index: oai_index, id, name }) => {
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

                let block_idx = next_block_index;
                current_tool_indices.insert(oai_index, block_idx);

                let block_start = ContentBlockStartEvent {
                    event_type: "content_block_start".into(),
                    index: block_idx,
                    content_block: ContentBlock::ToolUse {
                        id,
                        name,
                        input: serde_json::json!({}),
                        cache_control: None,
                    },
                };
                events.push(
                    Event::default()
                        .event("content_block_start")
                        .data(serde_json::to_string(&block_start).unwrap()),
                );
            }
            Ok(StreamEvent::ToolCallDelta { index: oai_index, arguments_delta }) => {
                let block_idx = current_tool_indices.get(&oai_index).copied().unwrap_or(oai_index);
                let delta_event = ContentBlockDeltaEvent {
                    event_type: "content_block_delta".into(),
                    index: block_idx,
                    delta: DeltaBlock::InputJsonDelta { partial_json: arguments_delta },
                };
                events.push(
                    Event::default()
                        .event("content_block_delta")
                        .data(serde_json::to_string(&delta_event).unwrap()),
                );
            }
            Ok(StreamEvent::Done { usage, finish_reason }) => {
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

                if let Some(ref u) = usage {
                    output_tokens = u.completion_tokens;
                    record_usage_fields(&span, u);
                }

                let stop_reason = finish_reason.map(|r| match r {
                    lortex_core::provider::FinishReason::Stop => "end_turn",
                    lortex_core::provider::FinishReason::ToolCalls => "tool_use",
                    lortex_core::provider::FinishReason::Length => "max_tokens",
                    lortex_core::provider::FinishReason::ContentFilter => "end_turn",
                }.to_string()).unwrap_or_else(|| "end_turn".into());

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

                let msg_stop = MessageStopEvent {
                    event_type: "message_stop".into(),
                };
                events.push(
                    Event::default()
                        .event("message_stop")
                        .data(serde_json::to_string(&msg_stop).unwrap()),
                );
            }
            Err(e) => {
                let err = AnthropicError::new("error", "api_error", e.to_string());
                events.push(
                    Event::default()
                        .event("error")
                        .data(serde_json::to_string(&err).unwrap()),
                );
            }
        }

        events
    });

    // Flatten Vec<Event> into individual events, wrap in Ok for Sse
    let flat_stream = main_stream.flat_map(|events| futures::stream::iter(events));
    let full_stream = flat_stream.map(Ok);

    Ok(Sse::new(full_stream).keep_alive(KeepAlive::default()))
}

// ============================================================================
// Passthrough 路径 — Anthropic 同格式透传
// ============================================================================

async fn passthrough_blocking_anthropic(
    state: &AppState,
    api_key: &ApiKey,
    model: &crate::models::Model,
    config: passthrough::PassthroughConfig,
    body: Vec<u8>,
) -> Result<Response, ProxyError> {
    let estimated_chars = body.len() as u64;

    let span = tracing::info_span!(
        target: "lortex::usage",
        "proxy_request",
        api_key_id = %api_key.id,
        api_key_name = %api_key.name,
        endpoint = "/v1/messages",
        stream = false,
        passthrough = true,
        estimated_chars,
        model_id = tracing::field::Empty,
        provider_id = tracing::field::Empty,
        vendor_model_name = tracing::field::Empty,
        input_tokens = tracing::field::Empty,
        output_tokens = tracing::field::Empty,
        cache_write_tokens = tracing::field::Empty,
        cache_read_tokens = tracing::field::Empty,
    );
    record_model_fields(&span, model);
    state.rate_limiter.record_model_request(&model.id());

    let (status, resp_bytes) = forward_blocking(&state.http_client, &config, body).await?;

    let _ = if status >= 400 {
        state
            .circuit_breaker
            .record_failure(&model.id())
            .await
    } else {
        state
            .circuit_breaker
            .record_success(&model.id())
            .await
    };

    if status >= 400 {
        tracing::warn!(
            status,
            endpoint = "/v1/messages",
            passthrough = true,
            provider = %model.provider_id,
            upstream_url = %config.upstream_url,
            "Upstream error: {}",
            String::from_utf8_lossy(&resp_bytes).chars().take(500).collect::<String>()
        );
    } else if let Ok(resp_value) = serde_json::from_slice::<Value>(&resp_bytes) {
        if let Some(usage) = extract_usage_anthropic(&resp_value) {
            record_usage_fields(&span, &usage);
        }
    }

    Ok((
        axum::http::StatusCode::from_u16(status)
            .unwrap_or(axum::http::StatusCode::BAD_GATEWAY),
        [(axum::http::header::CONTENT_TYPE, "application/json")],
        resp_bytes,
    )
        .into_response())
}

async fn passthrough_stream_anthropic(
    state: &AppState,
    api_key: &ApiKey,
    model: &crate::models::Model,
    config: passthrough::PassthroughConfig,
    body: Vec<u8>,
) -> Result<Response, ProxyError> {
    let estimated_chars = body.len() as u64;

    let span = tracing::info_span!(
        target: "lortex::usage",
        "proxy_request",
        api_key_id = %api_key.id,
        api_key_name = %api_key.name,
        endpoint = "/v1/messages",
        stream = true,
        passthrough = true,
        estimated_chars,
        model_id = tracing::field::Empty,
        provider_id = tracing::field::Empty,
        vendor_model_name = tracing::field::Empty,
        input_tokens = tracing::field::Empty,
        output_tokens = tracing::field::Empty,
        cache_write_tokens = tracing::field::Empty,
        cache_read_tokens = tracing::field::Empty,
    );
    record_model_fields(&span, model);
    state.rate_limiter.record_model_request(&model.id());

    let stream_result = forward_stream(&state.http_client, &config, body).await;

    match stream_result {
        Err(e) => {
            let _ = state
                .circuit_breaker
                .record_failure(&model.id())
                .await;
            tracing::warn!(
                status = %e.status,
                endpoint = "/v1/messages",
                passthrough = true,
                provider = %model.provider_id,
                upstream_url = %config.upstream_url,
                "Upstream stream error: {}",
                e.message.chars().take(500).collect::<String>()
            );
            Ok((
                e.status,
                [(axum::http::header::CONTENT_TYPE, "application/json")],
                axum::body::Bytes::from(e.message),
            )
                .into_response())
        }
        Ok((_status, sniffer_stream)) => {
            let _ = state
                .circuit_breaker
                .record_success(&model.id())
                .await;

            let usage_handle = sniffer_stream.usage_handle();

            let byte_stream = sniffer_stream.map(move |chunk| {
                chunk.map(axum::body::Bytes::from)
            });

            let body = axum::body::Body::from_stream(byte_stream);

            tokio::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                if let Ok(guard) = usage_handle.lock() {
                    if let Some(ref usage) = *guard {
                        record_usage_fields(&span, usage);
                    }
                }
            });

            Ok(Response::builder()
                .status(axum::http::StatusCode::OK)
                .header("Content-Type", "text/event-stream")
                .header("Cache-Control", "no-cache")
                .body(body)
                .unwrap())
        }
    }
}
