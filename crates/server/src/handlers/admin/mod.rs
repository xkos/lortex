//! Admin API handlers

use axum::http::StatusCode;

pub mod health;
pub mod keys;
pub mod models;
pub mod providers;
pub mod usage;

/// 把任何 `Result<T, E: Display>` 里的内部错误打印到日志并转成 500，
/// 避免 handler 里写 `.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)` 吞掉错误内容。
pub(crate) trait LogInternal<T> {
    fn log_internal(self, context: &'static str) -> Result<T, StatusCode>;
}

impl<T, E: std::fmt::Display> LogInternal<T> for Result<T, E> {
    fn log_internal(self, context: &'static str) -> Result<T, StatusCode> {
        self.map_err(|e| {
            tracing::error!(error = %e, context, "admin handler internal error");
            StatusCode::INTERNAL_SERVER_ERROR
        })
    }
}
