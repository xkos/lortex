//! Model — 模型配置（能力声明 + 计费倍率）

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 模型类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelType {
    Chat,
    Embedding,
    ImageGeneration,
    Tts,
    Stt,
}

impl ModelType {
    pub fn as_str(&self) -> &str {
        match self {
            ModelType::Chat => "chat",
            ModelType::Embedding => "embedding",
            ModelType::ImageGeneration => "image_generation",
            ModelType::Tts => "tts",
            ModelType::Stt => "stt",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "chat" => ModelType::Chat,
            "embedding" => ModelType::Embedding,
            "image_generation" => ModelType::ImageGeneration,
            "tts" => ModelType::Tts,
            "stt" => ModelType::Stt,
            _ => ModelType::Chat,
        }
    }
}

/// 模型配置，通过 provider_id 隐式关联到 Provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    /// 所属 Provider ID
    pub provider_id: String,
    /// 厂商侧的实际模型名
    pub vendor_model_name: String,
    /// 显示名称
    pub display_name: String,
    /// 短名称别名
    pub aliases: Vec<String>,
    /// 模型类型
    pub model_type: ModelType,

    // --- 能力声明 ---
    pub supports_streaming: bool,
    pub supports_tools: bool,
    pub supports_structured_output: bool,
    pub supports_vision: bool,
    pub supports_prefill: bool,
    pub supports_cache: bool,
    pub supports_web_search: bool,
    pub supports_batch: bool,
    pub context_window: u32,

    /// 缓存控制（默认开启）
    pub cache_enabled: bool,

    // --- 文本计费倍率（每 1k tokens 消耗的 credits）---
    pub input_multiplier: f64,
    pub output_multiplier: f64,
    pub cache_write_multiplier: Option<f64>,
    pub cache_read_multiplier: Option<f64>,

    // --- 多模态计费倍率（None = 不支持该模态）---
    pub image_input_multiplier: Option<f64>,
    pub audio_input_multiplier: Option<f64>,
    pub video_input_multiplier: Option<f64>,
    pub image_generation_multiplier: Option<f64>,
    pub tts_multiplier: Option<f64>,

    /// 自定义 header（转发请求时自动附加）
    pub extra_headers: HashMap<String, String>,

    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

impl Model {
    /// 模型 ID = "provider_id/vendor_model_name"
    pub fn id(&self) -> String {
        format!("{}/{}", self.provider_id, self.vendor_model_name)
    }

    /// 检查给定名称是否匹配此模型（ID 或任意别名）
    pub fn matches(&self, name: &str) -> bool {
        self.id() == name || self.aliases.iter().any(|a| a == name)
    }
}
