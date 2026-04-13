//! Provider 健康状态模型

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 熔断器状态
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CircuitState {
    /// 正常，允许所有请求
    Closed,
    /// 熔断，拒绝所有请求直到冷却期结束
    Open,
    /// 半开，允许探测请求
    HalfOpen,
}

impl Default for CircuitState {
    fn default() -> Self {
        Self::Closed
    }
}

/// Provider 级别的健康状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderHealthStatus {
    /// Provider ID
    pub provider_id: String,

    /// 当前熔断状态
    #[serde(default)]
    pub circuit_state: CircuitState,

    /// 连续失败次数
    #[serde(default)]
    pub consecutive_failures: u32,

    /// 最后一次成功时间
    pub last_success_at: Option<DateTime<Utc>>,

    /// 最后一次失败时间
    pub last_failure_at: Option<DateTime<Utc>>,

    /// 熔断开启时间（Open 状态的起点，用于计算冷却期）
    pub opened_at: Option<DateTime<Utc>>,

    /// 最后检查时间
    pub last_check_at: DateTime<Utc>,
}

impl ProviderHealthStatus {
    /// 创建一个健康的初始状态
    pub fn new(provider_id: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            circuit_state: CircuitState::Closed,
            consecutive_failures: 0,
            last_success_at: None,
            last_failure_at: None,
            opened_at: None,
            last_check_at: Utc::now(),
        }
    }
}
