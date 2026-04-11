//! UsageRecord — 用量记录

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 单次 LLM 调用的用量记录
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    pub id: String,
    pub api_key_id: String,
    pub api_key_name: String,
    pub provider_id: String,
    pub vendor_model_name: String,

    /// 请求入口格式
    pub request_endpoint: String, // "/v1/chat/completions" or "/v1/messages"

    // token 分类
    pub input_tokens: u32,
    #[serde(default)]
    pub cache_write_tokens: u32,
    #[serde(default)]
    pub cache_read_tokens: u32,
    pub output_tokens: u32,

    // 多模态用量（预留）
    #[serde(default)]
    pub image_input_units: u32,
    #[serde(default)]
    pub audio_input_seconds: f64,

    /// 消耗的 credits
    pub credits_consumed: i64,

    /// 请求耗时（毫秒）
    #[serde(default)]
    pub latency_ms: u64,

    pub created_at: DateTime<Utc>,
}

impl UsageRecord {
    pub fn model_id(&self) -> String {
        format!("{}/{}", self.provider_id, self.vendor_model_name)
    }
}
