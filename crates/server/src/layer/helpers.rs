//! Handler 辅助函数 — 将 Model / Usage 数据写入 tracing Span

use crate::models::Model;

/// 将模型信息写入当前 span
pub fn record_model_fields(span: &tracing::Span, model: &Model) {
    span.record("model_id", model.id().as_str());
    span.record("provider_id", model.provider_id.as_str());
    span.record("vendor_model_name", model.vendor_model_name.as_str());
    span.record("input_multiplier", model.input_multiplier);
    span.record("output_multiplier", model.output_multiplier);
    span.record("cache_write_multiplier", model.cache_write_multiplier.unwrap_or(0.0));
    span.record("cache_read_multiplier", model.cache_read_multiplier.unwrap_or(0.0));
}

/// 将 LLM usage 数据写入当前 span
pub fn record_usage_fields(span: &tracing::Span, usage: &lortex_core::provider::Usage) {
    span.record("input_tokens", usage.prompt_tokens as u64);
    span.record("output_tokens", usage.completion_tokens as u64);
    span.record("cache_write_tokens", usage.cache_creation_input_tokens as u64);
    span.record("cache_read_tokens", usage.cache_read_input_tokens as u64);
}
