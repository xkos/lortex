//! ApiKey — 代理密钥（租户隔离 + 额度控制）

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

    /// 额度上限（0 = 不限制）
    pub credit_limit: i64,
    /// 已消耗额度（只增不减，手动重置接口可清零）
    pub credit_used: i64,

    /// RPM 上限（0 = 不限制）
    #[serde(default)]
    pub rpm_limit: u32,
    /// TPM 上限（0 = 不限制）
    #[serde(default)]
    pub tpm_limit: u32,

    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

impl ApiKey {
    /// 计算剩余额度，None 表示不限制
    pub fn remaining_credits(&self) -> Option<i64> {
        if self.credit_limit == 0 {
            None
        } else {
            Some(self.credit_limit - self.credit_used)
        }
    }

    /// 是否还有额度
    pub fn has_credits(&self) -> bool {
        self.credit_limit == 0 || self.credit_used < self.credit_limit
    }

    /// 生成新的 proxy API key
    pub fn generate_key() -> String {
        format!("sk-proxy-{}", uuid::Uuid::new_v4().to_string().replace('-', ""))
    }
}
