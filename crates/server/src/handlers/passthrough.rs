//! 同格式透传 — 当客户端格式与模型 api_format 一致时，只做 auth+model 替换

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use bytes::Bytes;
use futures::{Stream, StreamExt};
use serde_json::Value;

use lortex_core::provider::Usage;

use crate::handlers::shared::ProxyError;
use crate::models::model::ApiFormat;
use crate::models::provider::AuthScheme;

const ANTHROPIC_API_VERSION: &str = "2023-06-01";

pub(crate) struct PassthroughConfig {
    pub upstream_url: String,
    pub api_key: String,
    pub format: ApiFormat,
    pub auth_scheme: AuthScheme,
    pub vendor_model_name: String,
    pub extra_headers: HashMap<String, String>,
}

/// 把 `Auto` 按 ApiFormat 解析成具体 scheme
fn resolve_auth_scheme(scheme: AuthScheme, format: &ApiFormat) -> AuthScheme {
    match scheme {
        AuthScheme::Auto => match format {
            ApiFormat::Anthropic => AuthScheme::XApiKey,
            ApiFormat::OpenAI => AuthScheme::Bearer,
        },
        other => other,
    }
}

pub(crate) fn prepare_body(
    raw: &[u8],
    config: &PassthroughConfig,
) -> Result<Vec<u8>, ProxyError> {
    let mut body: Value = serde_json::from_slice(raw)
        .map_err(|e| ProxyError::internal(format!("Invalid JSON body: {e}")))?;

    body["model"] = Value::String(config.vendor_model_name.clone());

    serde_json::to_vec(&body)
        .map_err(|e| ProxyError::internal(format!("Failed to serialize body: {e}")))
}

fn build_upstream_request(
    client: &reqwest::Client,
    config: &PassthroughConfig,
    body: Vec<u8>,
) -> reqwest::RequestBuilder {
    let mut req = client
        .post(&config.upstream_url)
        .header("Content-Type", "application/json")
        .body(body);

    // Auth header — 按 provider 配置的 scheme 决定；Auto 时按 format fallback
    req = match resolve_auth_scheme(config.auth_scheme, &config.format) {
        AuthScheme::Bearer => {
            req.header("Authorization", format!("Bearer {}", config.api_key))
        }
        AuthScheme::XApiKey => req.header("x-api-key", &config.api_key),
        AuthScheme::Auto => unreachable!("Auto resolved above"),
    };

    // Format-specific 非认证头
    if matches!(config.format, ApiFormat::Anthropic)
        && !config.extra_headers.contains_key("anthropic-version")
    {
        req = req.header("anthropic-version", ANTHROPIC_API_VERSION);
    }

    for (k, v) in &config.extra_headers {
        req = req.header(k.as_str(), v.as_str());
    }

    req
}

fn format_error_chain(e: &(dyn std::error::Error + 'static)) -> String {
    let mut msg = format!("{e}");
    let mut src = e.source();
    while let Some(s) = src {
        msg.push_str(&format!(" | caused by: {s}"));
        src = s.source();
    }
    msg
}

pub(crate) async fn forward_blocking(
    client: &reqwest::Client,
    config: &PassthroughConfig,
    body: Vec<u8>,
) -> Result<(u16, Bytes), ProxyError> {
    let resp = build_upstream_request(client, config, body)
        .send()
        .await
        .map_err(|e| {
            ProxyError::internal(format!("Upstream network error: {}", format_error_chain(&e)))
        })?;

    let status = resp.status().as_u16();
    let bytes = resp.bytes().await.map_err(|e| {
        ProxyError::internal(format!("Failed to read upstream body: {}", format_error_chain(&e)))
    })?;

    Ok((status, bytes))
}

pub(crate) async fn forward_stream(
    client: &reqwest::Client,
    config: &PassthroughConfig,
    body: Vec<u8>,
) -> Result<(u16, UsageSnifferStream), ProxyError> {
    let resp = build_upstream_request(client, config, body)
        .send()
        .await
        .map_err(|e| {
            ProxyError::internal(format!("Upstream network error: {}", format_error_chain(&e)))
        })?;

    let status = resp.status().as_u16();

    if status >= 400 {
        let error_bytes = resp.bytes().await.map_err(|e| {
            ProxyError::internal(format!(
                "Failed to read error body: {}",
                format_error_chain(&e)
            ))
        })?;
        return Err(ProxyError {
            status: axum::http::StatusCode::from_u16(status)
                .unwrap_or(axum::http::StatusCode::BAD_GATEWAY),
            message: String::from_utf8_lossy(&error_bytes).to_string(),
        });
    }

    let usage = Arc::new(Mutex::new(None));
    let format = config.format.clone();
    let usage_clone = usage.clone();

    let inner = resp.bytes_stream();
    let stream = UsageSnifferStream {
        inner: Box::pin(inner.map(move |chunk| {
            chunk.map_err(|e| {
                std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format_error_chain(&e),
                )
            })
        })),
        format,
        usage: usage_clone,
        buffer: String::new(),
    };

    Ok((status, stream))
}

// ============================================================================
// Usage extraction
// ============================================================================

pub(crate) fn extract_usage_anthropic(body: &Value) -> Option<Usage> {
    let u = body.get("usage")?;
    let input = u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let output = u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    Some(Usage {
        prompt_tokens: input,
        completion_tokens: output,
        total_tokens: input + output,
        cache_creation_input_tokens: u
            .get("cache_creation_input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
        cache_read_input_tokens: u
            .get("cache_read_input_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as u32,
    })
}

pub(crate) fn extract_usage_openai(body: &Value) -> Option<Usage> {
    let u = body.get("usage")?;
    let prompt = u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let completion = u
        .get("completion_tokens")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let total = u.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let cached = u
        .get("prompt_tokens_details")
        .and_then(|d| d.get("cached_tokens"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    Some(Usage {
        prompt_tokens: prompt,
        completion_tokens: completion,
        total_tokens: total,
        cache_creation_input_tokens: 0,
        cache_read_input_tokens: cached,
    })
}

// ============================================================================
// UsageSnifferStream — 嗅探 SSE 中的 usage 但不修改字节流
// ============================================================================

pub(crate) struct UsageSnifferStream {
    inner: Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>,
    format: ApiFormat,
    usage: Arc<Mutex<Option<Usage>>>,
    buffer: String,
}

impl UsageSnifferStream {
    pub fn usage_handle(&self) -> Arc<Mutex<Option<Usage>>> {
        self.usage.clone()
    }

    fn sniff_line(&mut self, line: &str) {
        let data = match line.strip_prefix("data: ") {
            Some(d) if d != "[DONE]" => d,
            _ => return,
        };

        let parsed: Value = match serde_json::from_str(data) {
            Ok(v) => v,
            Err(_) => return,
        };

        match self.format {
            ApiFormat::Anthropic => {
                self.sniff_anthropic_event(&parsed);
            }
            ApiFormat::OpenAI => {
                self.sniff_openai_event(&parsed);
            }
        }
    }

    fn sniff_anthropic_event(&mut self, event: &Value) {
        let event_type = event.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match event_type {
            "message_start" => {
                if let Some(u) = event.get("message").and_then(|m| m.get("usage")) {
                    let input =
                        u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    let cache_create = u
                        .get("cache_creation_input_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    let cache_read = u
                        .get("cache_read_input_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as u32;
                    let mut guard = self.usage.lock().unwrap();
                    let usage = guard.get_or_insert(Usage {
                        prompt_tokens: 0,
                        completion_tokens: 0,
                        total_tokens: 0,
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 0,
                    });
                    usage.prompt_tokens = input;
                    usage.cache_creation_input_tokens = cache_create;
                    usage.cache_read_input_tokens = cache_read;
                }
            }
            "message_delta" => {
                if let Some(u) = event.get("usage") {
                    let output =
                        u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                    let mut guard = self.usage.lock().unwrap();
                    let usage = guard.get_or_insert(Usage {
                        prompt_tokens: 0,
                        completion_tokens: 0,
                        total_tokens: 0,
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 0,
                    });
                    usage.completion_tokens = output;
                    usage.total_tokens = usage.prompt_tokens + output;
                }
            }
            _ => {}
        }
    }

    fn sniff_openai_event(&mut self, chunk: &Value) {
        if let Some(u) = chunk.get("usage") {
            let total = u.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            if total > 0 {
                let prompt =
                    u.get("prompt_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
                let completion = u
                    .get("completion_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                let cached = u
                    .get("prompt_tokens_details")
                    .and_then(|d| d.get("cached_tokens"))
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u32;
                let mut guard = self.usage.lock().unwrap();
                *guard = Some(Usage {
                    prompt_tokens: prompt,
                    completion_tokens: completion,
                    total_tokens: total,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: cached,
                });
            }
        }
    }
}

impl Stream for UsageSnifferStream {
    type Item = Result<Bytes, std::io::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();
        let result = Pin::new(&mut this.inner).poll_next(cx);

        if let Poll::Ready(Some(Ok(ref bytes))) = result {
            this.buffer.push_str(&String::from_utf8_lossy(bytes));
            while let Some(newline_pos) = this.buffer.find('\n') {
                let line = this.buffer[..newline_pos]
                    .trim_end_matches('\r')
                    .to_string();
                this.buffer = this.buffer[newline_pos + 1..].to_string();
                if !line.is_empty() {
                    this.sniff_line(&line);
                }
            }
        }

        result
    }
}

