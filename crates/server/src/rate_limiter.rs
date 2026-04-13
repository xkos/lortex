//! 滑动窗口 Rate Limiter — 按 ApiKey 维度的 RPM / TPM 限流

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use dashmap::DashMap;

const WINDOW: Duration = Duration::from_secs(60);

/// 每个 ApiKey 的 RPM / TPM 滑动窗口计数器
pub struct RateLimiter {
    /// key_id → 请求时间戳队列
    rpm: DashMap<String, VecDeque<Instant>>,
    /// key_id → (时间戳, token 数) 队列
    tpm: DashMap<String, VecDeque<(Instant, u32)>>,
}

/// RPM 检查结果
#[derive(Debug)]
pub struct RpmStatus {
    pub remaining: u32,
    pub reset_after: Duration,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            rpm: DashMap::new(),
            tpm: DashMap::new(),
        }
    }

    /// 检查并记录一次请求。返回 Ok(剩余次数) 或 Err(重置等待时间)。
    /// limit=0 表示不限制，直接返回 Ok。
    pub fn check_rpm(&self, key_id: &str, limit: u32) -> Result<RpmStatus, Duration> {
        if limit == 0 {
            return Ok(RpmStatus {
                remaining: u32::MAX,
                reset_after: Duration::ZERO,
            });
        }

        let now = Instant::now();
        let mut entry = self.rpm.entry(key_id.to_owned()).or_default();
        let window = &mut *entry;

        // 清理过期条目
        while let Some(front) = window.front() {
            if now.duration_since(*front) > WINDOW {
                window.pop_front();
            } else {
                break;
            }
        }

        let count = window.len() as u32;
        if count >= limit {
            // 超限：返回最早条目过期的等待时间
            let reset_after = WINDOW
                .checked_sub(now.duration_since(*window.front().unwrap()))
                .unwrap_or(Duration::ZERO);
            return Err(reset_after);
        }

        // 记录本次请求
        window.push_back(now);
        Ok(RpmStatus {
            remaining: limit - count - 1,
            reset_after: WINDOW
                .checked_sub(now.duration_since(*window.front().unwrap()))
                .unwrap_or(WINDOW),
        })
    }

    /// 检查 TPM 窗口内累计 token 数。返回 Ok(剩余 token) 或 Err(重置等待时间)。
    /// limit=0 表示不限制。
    pub fn check_tpm(&self, key_id: &str, limit: u32) -> Result<u32, Duration> {
        if limit == 0 {
            return Ok(u32::MAX);
        }

        let now = Instant::now();
        let mut entry = self.tpm.entry(key_id.to_owned()).or_default();
        let window = &mut *entry;

        // 清理过期条目
        while let Some((ts, _)) = window.front() {
            if now.duration_since(*ts) > WINDOW {
                window.pop_front();
            } else {
                break;
            }
        }

        let total: u32 = window.iter().map(|(_, t)| t).sum();
        if total >= limit {
            let reset_after = window
                .front()
                .map(|(ts, _)| {
                    WINDOW
                        .checked_sub(now.duration_since(*ts))
                        .unwrap_or(Duration::ZERO)
                })
                .unwrap_or(Duration::ZERO);
            return Err(reset_after);
        }

        Ok(limit - total)
    }

    /// 记录已完成请求的 token 使用量（由 UsageLayer 调用）
    pub fn record_tokens(&self, key_id: &str, tokens: u32) {
        if tokens == 0 {
            return;
        }
        let now = Instant::now();
        self.tpm
            .entry(key_id.to_owned())
            .or_default()
            .push_back((now, tokens));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rpm_unlimited() {
        let rl = RateLimiter::new();
        for _ in 0..100 {
            assert!(rl.check_rpm("k1", 0).is_ok());
        }
    }

    #[test]
    fn rpm_within_limit() {
        let rl = RateLimiter::new();
        for i in 0..5 {
            let status = rl.check_rpm("k1", 5).unwrap();
            assert_eq!(status.remaining, 4 - i);
        }
    }

    #[test]
    fn rpm_exceeds_limit() {
        let rl = RateLimiter::new();
        for _ in 0..3 {
            assert!(rl.check_rpm("k1", 3).is_ok());
        }
        let err = rl.check_rpm("k1", 3).unwrap_err();
        assert!(err <= WINDOW);
    }

    #[test]
    fn rpm_per_key_isolation() {
        let rl = RateLimiter::new();
        for _ in 0..3 {
            assert!(rl.check_rpm("k1", 3).is_ok());
        }
        assert!(rl.check_rpm("k1", 3).is_err());
        // k2 is independent
        assert!(rl.check_rpm("k2", 3).is_ok());
    }

    #[test]
    fn tpm_unlimited() {
        let rl = RateLimiter::new();
        rl.record_tokens("k1", 999999);
        assert!(rl.check_tpm("k1", 0).is_ok());
    }

    #[test]
    fn tpm_within_limit() {
        let rl = RateLimiter::new();
        rl.record_tokens("k1", 500);
        let remaining = rl.check_tpm("k1", 1000).unwrap();
        assert_eq!(remaining, 500);
    }

    #[test]
    fn tpm_exceeds_limit() {
        let rl = RateLimiter::new();
        rl.record_tokens("k1", 800);
        rl.record_tokens("k1", 300);
        let err = rl.check_tpm("k1", 1000).unwrap_err();
        assert!(err <= WINDOW);
    }

    #[test]
    fn tpm_zero_tokens_not_recorded() {
        let rl = RateLimiter::new();
        rl.record_tokens("k1", 0);
        assert!(!rl.tpm.contains_key("k1"));
    }

    #[test]
    fn rpm_window_expires() {
        let rl = RateLimiter::new();

        // 手动插入一个已过期的时间戳
        {
            let mut entry = rl.rpm.entry("k1".to_owned()).or_default();
            entry.push_back(Instant::now() - WINDOW - Duration::from_secs(1));
        }

        // 过期条目应被清理，不占计数
        let status = rl.check_rpm("k1", 1).unwrap();
        assert_eq!(status.remaining, 0);
    }

    #[test]
    fn tpm_window_expires() {
        let rl = RateLimiter::new();

        // 手动插入一个已过期的 token 记录
        {
            let mut entry = rl.tpm.entry("k1".to_owned()).or_default();
            entry.push_back((Instant::now() - WINDOW - Duration::from_secs(1), 9999));
        }

        // 过期条目应被清理
        let remaining = rl.check_tpm("k1", 1000).unwrap();
        assert_eq!(remaining, 1000);
    }
}
