//! ApiKey — 代理密钥（租户隔离 + 额度控制）

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 客户端接入 proxy 使用的密钥
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    /// 内部 UUID
    pub id: String,
    /// 对客户端可见的密钥（"sk-proxy-" 前缀 + 随机串）
    pub key: String,
    /// 可读名称
    pub name: String,

    /// 可用模型 ID 列表（支持 ID 或别名）
    pub model_group: Vec<String>,
    /// PROXY_MANAGED 时的首选模型
    pub default_model: String,
    /// 故障时的备选顺序（Phase 2 实现）
    pub fallback_models: Vec<String>,

    /// RPM 上限（0 = 不限制）
    #[serde(default)]
    pub rpm_limit: u32,
    /// TPM 上限（0 = 不限制）
    #[serde(default)]
    pub tpm_limit: u32,

    /// 模型映射（客户端模型名 → 实际模型 ID）
    /// 例: "claude-sonnet-4-6" → "anthropic/claude-sonnet-4-6"
    #[serde(default)]
    pub model_map: HashMap<String, String>,

    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

impl ApiKey {
    /// 生成新的 proxy API key
    pub fn generate_key() -> String {
        format!("sk-proxy-{}", uuid::Uuid::new_v4().to_string().replace('-', ""))
    }
}
