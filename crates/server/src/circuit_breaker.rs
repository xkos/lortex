//! 熔断器 — Model 级别的健康检测与自动恢复
//!
//! 状态转换：
//!   Closed → (连续 failure_threshold 次失败) → Open
//!   Open   → (冷却 cooldown 秒后) → HalfOpen
//!   HalfOpen → (一次成功) → Closed
//!   HalfOpen → (一次失败) → Open

use std::sync::Arc;

use chrono::Utc;

use crate::models::health::{CircuitState, ModelHealthStatus};
use crate::store::traits::ProxyStore;
use crate::store::StoreError;

/// 熔断器配置
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// 连续失败多少次后触发熔断
    pub failure_threshold: u32,
    /// 熔断冷却时间（秒）
    pub cooldown_secs: u64,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 3,
            cooldown_secs: 30,
        }
    }
}

/// 熔断器服务
pub struct CircuitBreaker {
    store: Arc<dyn ProxyStore>,
    config: CircuitBreakerConfig,
}

impl CircuitBreaker {
    pub fn new(store: Arc<dyn ProxyStore>, config: CircuitBreakerConfig) -> Self {
        Self { store, config }
    }

    /// 判断模型是否可用（允许请求通过）
    ///
    /// - Closed → true
    /// - HalfOpen → true（允许探测）
    /// - Open → 检查冷却期，过期则转为 HalfOpen 并返回 true
    pub async fn is_available(&self, model_id: &str) -> Result<bool, StoreError> {
        let status = match self.store.get_health_status(model_id).await? {
            Some(s) => s,
            None => return Ok(true), // 无记录 = 健康
        };

        match status.circuit_state {
            CircuitState::Closed => Ok(true),
            CircuitState::HalfOpen => Ok(true),
            CircuitState::Open => {
                // 检查冷却期是否已过
                if let Some(opened_at) = status.opened_at {
                    let elapsed = (Utc::now() - opened_at).num_seconds();
                    if elapsed >= self.config.cooldown_secs as i64 {
                        // 冷却期过，转为 HalfOpen
                        let mut updated = status;
                        updated.circuit_state = CircuitState::HalfOpen;
                        updated.last_check_at = Utc::now();
                        self.store.upsert_health_status(&updated).await?;
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
    }

    /// 记录一次成功请求
    pub async fn record_success(&self, model_id: &str) -> Result<(), StoreError> {
        let mut status = self
            .store
            .get_health_status(model_id)
            .await?
            .unwrap_or_else(|| ModelHealthStatus::new(model_id));

        status.circuit_state = CircuitState::Closed;
        status.consecutive_failures = 0;
        status.last_success_at = Some(Utc::now());
        status.opened_at = None;
        status.last_check_at = Utc::now();

        self.store.upsert_health_status(&status).await
    }

    /// 强制重置熔断器（管理员手动恢复）
    pub async fn force_reset(&self, model_id: &str) -> Result<(), StoreError> {
        let status = ModelHealthStatus::new(model_id);
        self.store.upsert_health_status(&status).await
    }

    /// 记录一次失败请求
    pub async fn record_failure(&self, model_id: &str) -> Result<(), StoreError> {
        let mut status = self
            .store
            .get_health_status(model_id)
            .await?
            .unwrap_or_else(|| ModelHealthStatus::new(model_id));

        status.consecutive_failures += 1;
        status.last_failure_at = Some(Utc::now());
        status.last_check_at = Utc::now();

        // 状态转换
        match status.circuit_state {
            CircuitState::Closed => {
                if status.consecutive_failures >= self.config.failure_threshold {
                    status.circuit_state = CircuitState::Open;
                    status.opened_at = Some(Utc::now());
                }
            }
            CircuitState::HalfOpen => {
                // 探测失败，重新打开
                status.circuit_state = CircuitState::Open;
                status.opened_at = Some(Utc::now());
            }
            CircuitState::Open => {
                // 已经 Open，更新时间戳即可
            }
        }

        self.store.upsert_health_status(&status).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::SqliteStore;

    async fn test_cb() -> CircuitBreaker {
        let store = SqliteStore::new(":memory:").await.unwrap();
        store.migrate().await.unwrap();
        CircuitBreaker::new(
            Arc::new(store),
            CircuitBreakerConfig {
                failure_threshold: 3,
                cooldown_secs: 1, // 1秒冷却，方便测试
            },
        )
    }

    #[tokio::test]
    async fn initially_available() {
        let cb = test_cb().await;
        assert!(cb.is_available("openai/gpt-4o").await.unwrap());
    }

    #[tokio::test]
    async fn stays_closed_below_threshold() {
        let cb = test_cb().await;
        cb.record_failure("openai/gpt-4o").await.unwrap();
        cb.record_failure("openai/gpt-4o").await.unwrap();
        // 2 failures, threshold is 3 → still available
        assert!(cb.is_available("openai/gpt-4o").await.unwrap());
    }

    #[tokio::test]
    async fn opens_at_threshold() {
        let cb = test_cb().await;
        for _ in 0..3 {
            cb.record_failure("openai/gpt-4o").await.unwrap();
        }
        assert!(!cb.is_available("openai/gpt-4o").await.unwrap());
    }

    #[tokio::test]
    async fn success_resets_failures() {
        let cb = test_cb().await;
        cb.record_failure("openai/gpt-4o").await.unwrap();
        cb.record_failure("openai/gpt-4o").await.unwrap();
        cb.record_success("openai/gpt-4o").await.unwrap();
        // Reset, so 3 more failures needed
        cb.record_failure("openai/gpt-4o").await.unwrap();
        assert!(cb.is_available("openai/gpt-4o").await.unwrap());
    }

    #[tokio::test]
    async fn half_open_after_cooldown() {
        let cb = test_cb().await;
        for _ in 0..3 {
            cb.record_failure("openai/gpt-4o").await.unwrap();
        }
        assert!(!cb.is_available("openai/gpt-4o").await.unwrap());

        // Wait for cooldown (1 second)
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        // Should transition to HalfOpen and be available
        assert!(cb.is_available("openai/gpt-4o").await.unwrap());

        // Verify state is now HalfOpen
        let status = cb.store.get_health_status("openai/gpt-4o").await.unwrap().unwrap();
        assert_eq!(status.circuit_state, CircuitState::HalfOpen);
    }

    #[tokio::test]
    async fn half_open_success_closes() {
        let cb = test_cb().await;
        for _ in 0..3 {
            cb.record_failure("openai/gpt-4o").await.unwrap();
        }
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        cb.is_available("openai/gpt-4o").await.unwrap(); // triggers HalfOpen

        cb.record_success("openai/gpt-4o").await.unwrap();
        let status = cb.store.get_health_status("openai/gpt-4o").await.unwrap().unwrap();
        assert_eq!(status.circuit_state, CircuitState::Closed);
        assert_eq!(status.consecutive_failures, 0);
    }

    #[tokio::test]
    async fn half_open_failure_reopens() {
        let cb = test_cb().await;
        for _ in 0..3 {
            cb.record_failure("openai/gpt-4o").await.unwrap();
        }
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
        cb.is_available("openai/gpt-4o").await.unwrap(); // triggers HalfOpen

        cb.record_failure("openai/gpt-4o").await.unwrap();
        let status = cb.store.get_health_status("openai/gpt-4o").await.unwrap().unwrap();
        assert_eq!(status.circuit_state, CircuitState::Open);
    }

    #[tokio::test]
    async fn providers_independent() {
        let cb = test_cb().await;
        for _ in 0..3 {
            cb.record_failure("openai/gpt-4o").await.unwrap();
        }
        assert!(!cb.is_available("openai/gpt-4o").await.unwrap());
        assert!(cb.is_available("anthropic/claude-3").await.unwrap());
    }
}
