//! 启动配置

use serde::{Deserialize, Serialize};

/// Proxy 服务启动配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// 主端口（proxy 入口）
    pub port: u16,
    /// 主机地址
    pub host: String,
    /// admin API 端口，None 表示与主端口合并
    pub admin_port: Option<u16>,
    /// SQLite 数据库文件路径
    pub db_path: String,
    /// admin API 鉴权 key
    pub admin_key: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            host: "127.0.0.1".into(),
            admin_port: None,
            db_path: "lortex.db".into(),
            admin_key: String::new(),
        }
    }
}

impl ServerConfig {
    /// 从环境变量读取配置，缺失项使用默认值
    pub fn from_env() -> Self {
        Self {
            port: std::env::var("LORTEX_PORT")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(8080),
            host: std::env::var("LORTEX_HOST").unwrap_or_else(|_| "127.0.0.1".into()),
            admin_port: std::env::var("LORTEX_ADMIN_PORT")
                .ok()
                .and_then(|s| s.parse().ok()),
            db_path: std::env::var("LORTEX_DB").unwrap_or_else(|_| "lortex.db".into()),
            admin_key: std::env::var("LORTEX_ADMIN_KEY").unwrap_or_default(),
        }
    }
}
