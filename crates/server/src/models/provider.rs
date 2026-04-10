//! Provider — LLM 服务供应商配置

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 厂商类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Vendor {
    OpenAI,
    Anthropic,
    DeepSeek,
    /// 其他兼容 OpenAI 格式的厂商
    Custom(String),
}

impl Vendor {
    pub fn as_str(&self) -> &str {
        match self {
            Vendor::OpenAI => "openai",
            Vendor::Anthropic => "anthropic",
            Vendor::DeepSeek => "deepseek",
            Vendor::Custom(s) => s,
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "openai" => Vendor::OpenAI,
            "anthropic" => Vendor::Anthropic,
            "deepseek" => Vendor::DeepSeek,
            other => Vendor::Custom(other.to_string()),
        }
    }
}

/// LLM 服务供应商，持有访问凭证和 base URL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    /// 用户自定义唯一标识
    pub id: String,
    /// 厂商类型
    pub vendor: Vendor,
    /// 显示名称
    pub display_name: String,
    /// 厂商 API Key
    pub api_key: String,
    /// 厂商 API base URL，支持覆盖（中转场景）
    pub base_url: String,
    /// 是否启用
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}
