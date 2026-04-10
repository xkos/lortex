//! lortex-proxy — LLM 中转代理服务

use std::sync::Arc;

use clap::Parser;
use tracing_subscriber::EnvFilter;

use lortex_server::{app_router, AppState, ServerConfig};
use lortex_server::store::SqliteStore;

/// Lortex Proxy — 统一 LLM 接入网关
#[derive(Parser)]
#[command(name = "lortex-proxy", version, about)]
struct Cli {
    /// 监听端口
    #[arg(long, default_value = "8080", env = "LORTEX_PORT")]
    port: u16,

    /// 监听地址
    #[arg(long, default_value = "127.0.0.1", env = "LORTEX_HOST")]
    host: String,

    /// Admin API 独立端口（不设置则与主端口合并）
    #[arg(long, env = "LORTEX_ADMIN_PORT")]
    admin_port: Option<u16>,

    /// SQLite 数据库文件路径
    #[arg(long, default_value = "lortex.db", env = "LORTEX_DB")]
    db: String,

    /// Admin API 鉴权密钥
    #[arg(long, env = "LORTEX_ADMIN_KEY")]
    admin_key: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,lortex_server=debug")),
        )
        .init();

    let cli = Cli::parse();

    tracing::info!(
        host = %cli.host,
        port = cli.port,
        admin_port = ?cli.admin_port,
        db = %cli.db,
        "Starting Lortex Proxy"
    );

    // 初始化存储
    let store = SqliteStore::new(&cli.db).await?;
    store.migrate().await?;
    tracing::info!("Database initialized: {}", cli.db);

    let state = AppState {
        store: Arc::new(store),
    };

    let config = ServerConfig {
        port: cli.port,
        host: cli.host,
        admin_port: cli.admin_port,
        db_path: cli.db,
        admin_key: cli.admin_key.clone(),
    };

    // 构建路由
    let app = app_router(state.clone(), config.admin_key.clone());

    // 启动主服务
    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("Proxy listening on {}", addr);

    if let Some(admin_port) = config.admin_port {
        // Admin API 独立端口
        let admin_addr = format!("{}:{}", config.host, admin_port);
        let admin_listener = tokio::net::TcpListener::bind(&admin_addr).await?;
        tracing::info!("Admin API listening on {}", admin_addr);

        let admin_app = lortex_server::routes::admin_routes(state, config.admin_key);
        let admin_app = axum::Router::new().nest("/admin/v1", admin_app);

        // 主端口不含 admin 路由
        let main_app = axum::Router::new();
        // TODO: 003b 添加 proxy 路由

        tokio::select! {
            r = axum::serve(listener, main_app) => r?,
            r = axum::serve(admin_listener, admin_app) => r?,
        }
    } else {
        // 合并端口
        axum::serve(listener, app).await?;
    }

    Ok(())
}
