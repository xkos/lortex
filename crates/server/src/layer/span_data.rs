//! Span 数据容器 — 存储在 span extensions 中，由 UsageLayer 读写

use std::time::Instant;

/// 记录 span 创建时间，用于计算 latency_ms
pub(crate) struct SpanTiming {
    pub start: Instant,
}

/// 累积 span.record() 写入的所有 usage 相关字段
#[derive(Debug, Default)]
pub(crate) struct SpanData {
    // 请求上下文
    pub api_key_id: Option<String>,
    pub api_key_name: Option<String>,
    pub endpoint: Option<String>,
    pub stream: Option<bool>,
    pub estimated_chars: Option<u64>,

    // 模型信息
    pub model_id: Option<String>,
    pub provider_id: Option<String>,
    pub vendor_model_name: Option<String>,
    // Usage 数据（LLM 响应后填充）
    pub input_tokens: Option<u32>,
    pub output_tokens: Option<u32>,
    pub cache_write_tokens: Option<u32>,
    pub cache_read_tokens: Option<u32>,

    // 耗时（仅 streaming 路径由 handler 设置）
    pub ttft_ms: Option<u64>,
}

impl tracing::field::Visit for SpanData {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            "api_key_id" => self.api_key_id = Some(value.to_string()),
            "api_key_name" => self.api_key_name = Some(value.to_string()),
            "endpoint" => self.endpoint = Some(value.to_string()),
            "model_id" => self.model_id = Some(value.to_string()),
            "provider_id" => self.provider_id = Some(value.to_string()),
            "vendor_model_name" => self.vendor_model_name = Some(value.to_string()),
            _ => {}
        }
    }

    fn record_u64(&mut self, field: &tracing::field::Field, value: u64) {
        match field.name() {
            "estimated_chars" => self.estimated_chars = Some(value),
            "input_tokens" => self.input_tokens = Some(value as u32),
            "output_tokens" => self.output_tokens = Some(value as u32),
            "cache_write_tokens" => self.cache_write_tokens = Some(value as u32),
            "cache_read_tokens" => self.cache_read_tokens = Some(value as u32),
            "ttft_ms" => self.ttft_ms = Some(value),
            _ => {}
        }
    }

    fn record_i64(&mut self, field: &tracing::field::Field, value: i64) {
        // u32 token values might arrive as i64 via tracing
        match field.name() {
            "input_tokens" => self.input_tokens = Some(value as u32),
            "output_tokens" => self.output_tokens = Some(value as u32),
            "cache_write_tokens" => self.cache_write_tokens = Some(value as u32),
            "cache_read_tokens" => self.cache_read_tokens = Some(value as u32),
            _ => {}
        }
    }

    fn record_f64(&mut self, _field: &tracing::field::Field, _value: f64) {}

    fn record_bool(&mut self, field: &tracing::field::Field, value: bool) {
        match field.name() {
            "stream" => self.stream = Some(value),
            _ => {}
        }
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        // % (Display) 格式的字段通过 record_debug 传递，需要格式化后转发
        let s = format!("{:?}", value);
        self.record_str(field, &s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_data_default_is_all_none() {
        let data = SpanData::default();
        assert!(data.api_key_id.is_none());
        assert!(data.model_id.is_none());
        assert!(data.input_tokens.is_none());
        assert!(data.ttft_ms.is_none());
    }
}
