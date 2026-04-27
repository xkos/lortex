//! Provider — LLM 服务供应商配置

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 厂商类型
#[derive(Debug, Clone, PartialEq, Eq)]
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

impl Serialize for Vendor {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Vendor {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Vendor::from_str(&s))
    }
}

/// 上游认证方案
///
/// - `Auto`（默认）：按 ApiFormat 决定 —— Anthropic 走 `x-api-key`，OpenAI 走 `Authorization: Bearer`
/// - `Bearer`：强制 `Authorization: Bearer <key>`（常见于第三方 Anthropic 兼容网关，如 cursorlink、openrouter）
/// - `XApiKey`：强制 `x-api-key: <key>`（Anthropic 官方）
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthScheme {
    #[default]
    Auto,
    Bearer,
    XApiKey,
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
    /// 官网链接（方便快速跳转，尤其中转商场景）
    #[serde(default)]
    pub website_url: String,
    /// 上游认证方案，默认 Auto（按 ApiFormat 决定）
    #[serde(default)]
    pub auth_scheme: AuthScheme,
    /// 是否启用
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}
