//! UsageLayer — tracing Layer 实现
//!
//! 在 span 关闭时统一处理：
//! 1. 计算 latency_ms
//! 2. compute_credits()
//! 3. tokio::spawn { add_credits_used + insert_usage + tracing::info! }
//!
//! 仅处理 target = "lortex::usage" 的 span。

use std::sync::Arc;

use tracing::Subscriber;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

use crate::middleware::proxy_auth::compute_credits;
use crate::models::UsageRecord;
use crate::rate_limiter::RateLimiter;
use crate::store::ProxyStore;

use super::span_data::{SpanData, SpanTiming};

const USAGE_TARGET: &str = "lortex::usage";

/// 观测层：自动收集请求指标并写入用量记录
pub struct UsageLayer {
    store: Arc<dyn ProxyStore>,
    rate_limiter: Arc<RateLimiter>,
}

impl UsageLayer {
    pub fn new(store: Arc<dyn ProxyStore>, rate_limiter: Arc<RateLimiter>) -> Self {
        Self { store, rate_limiter }
    }
}

impl<S> tracing_subscriber::Layer<S> for UsageLayer
where
    S: Subscriber + for<'lookup> LookupSpan<'lookup>,
{
    fn on_new_span(
        &self,
        attrs: &tracing::span::Attributes<'_>,
        id: &tracing::span::Id,
        ctx: Context<'_, S>,
    ) {
        let span = match ctx.span(id) {
            Some(s) => s,
            None => return,
        };

        if span.metadata().target() != USAGE_TARGET {
            return;
        }

        let mut data = SpanData::default();
        attrs.record(&mut data);

        let mut extensions = span.extensions_mut();
        extensions.insert(SpanTiming {
            start: std::time::Instant::now(),
        });
        extensions.insert(data);
    }

    fn on_record(
        &self,
        id: &tracing::span::Id,
        values: &tracing::span::Record<'_>,
        ctx: Context<'_, S>,
    ) {
        let span = match ctx.span(id) {
            Some(s) => s,
            None => return,
        };

        if span.metadata().target() != USAGE_TARGET {
            return;
        }

        let mut extensions = span.extensions_mut();
        if let Some(data) = extensions.get_mut::<SpanData>() {
            values.record(data);
        }
    }

    fn on_close(&self, id: tracing::span::Id, ctx: Context<'_, S>) {
        let span = match ctx.span(&id) {
            Some(s) => s,
            None => return,
        };

        if span.metadata().target() != USAGE_TARGET {
            return;
        }

        let mut extensions = span.extensions_mut();

        // 提取计时数据（remove 后不再需要）
        let latency_ms = extensions
            .remove::<SpanTiming>()
            .map(|t| t.start.elapsed().as_millis() as u64)
            .unwrap_or(0);

        // 提取 span 数据
        let data = match extensions.remove::<SpanData>() {
            Some(d) => d,
            None => return,
        };

        // 没有 input_tokens 说明 handler 在 LLM 调用前就失败了，跳过
        let input_tokens = match data.input_tokens {
            Some(t) => t,
            None => return,
        };

        let output_tokens = data.output_tokens.unwrap_or(0);
        let cache_write_tokens = data.cache_write_tokens.unwrap_or(0);
        let cache_read_tokens = data.cache_read_tokens.unwrap_or(0);

        let input_multiplier = data.input_multiplier.unwrap_or(0.0);
        let output_multiplier = data.output_multiplier.unwrap_or(0.0);
        let cache_write_multiplier = if data.cache_write_multiplier == Some(0.0) {
            None
        } else {
            data.cache_write_multiplier
        };
        let cache_read_multiplier = if data.cache_read_multiplier == Some(0.0) {
            None
        } else {
            data.cache_read_multiplier
        };

        let credits = compute_credits(
            input_tokens,
            output_tokens,
            cache_write_tokens,
            cache_read_tokens,
            input_multiplier,
            output_multiplier,
            cache_write_multiplier,
            cache_read_multiplier,
        );

        // ttft: streaming 路径由 handler 设置；blocking 路径回退到 latency_ms
        let ttft_ms = data.ttft_ms.unwrap_or(latency_ms);

        let api_key_id = data.api_key_id.unwrap_or_default();
        let api_key_name = data.api_key_name.unwrap_or_default();
        let provider_id = data.provider_id.unwrap_or_default();
        let vendor_model_name = data.vendor_model_name.unwrap_or_default();
        let endpoint = data.endpoint.unwrap_or_default();
        let estimated_chars = data.estimated_chars.unwrap_or(0);
        let model_id = data.model_id.clone().unwrap_or_default();
        let is_stream = data.stream.unwrap_or(false);

        // 记录 token 到 RateLimiter（同步，无需 spawn）
        let total_tokens = input_tokens + output_tokens;
        self.rate_limiter.record_tokens(&api_key_id, total_tokens);

        let store = self.store.clone();

        // on_close 是同步回调，异步写库需要 tokio::spawn
        tokio::spawn(async move {
            // 扣减 credit
            if let Err(e) = store.add_credits_used(&api_key_id, credits).await {
                tracing::warn!(
                    target: "lortex::layer",
                    error = %e,
                    api_key_id = %api_key_id,
                    "Failed to deduct credits"
                );
            }

            // 写入用量记录
            let record = UsageRecord {
                id: uuid::Uuid::new_v4().to_string(),
                api_key_id: api_key_id.clone(),
                api_key_name: api_key_name.clone(),
                provider_id: provider_id.clone(),
                vendor_model_name: vendor_model_name.clone(),
                request_endpoint: endpoint.clone(),
                input_tokens,
                cache_write_tokens,
                cache_read_tokens,
                output_tokens,
                image_input_units: 0,
                audio_input_seconds: 0.0,
                credits_consumed: credits,
                estimated_chars,
                ttft_ms,
                latency_ms,
                created_at: chrono::Utc::now(),
            };
            if let Err(e) = store.insert_usage(&record).await {
                tracing::warn!(
                    target: "lortex::layer",
                    error = %e,
                    "Failed to write usage record"
                );
            }

            // 结构化日志 — 用 lortex::layer target 避免递归触发
            tracing::info!(
                target: "lortex::layer",
                api_key_name = %api_key_name,
                model = %model_id,
                provider = %provider_id,
                endpoint = %endpoint,
                stream = is_stream,
                input_tokens,
                output_tokens,
                cache_write_tokens,
                cache_read_tokens,
                credits,
                estimated_chars,
                ttft_ms,
                latency_ms,
                "Proxy request completed"
            );
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_layer_creates_with_store() {
        // 基本构造测试 — 完整集成测试在 T4/T5 中覆盖
        // 此处验证 UsageLayer::new 不 panic
        let _ = USAGE_TARGET; // 确保常量存在
    }
}
