//! SQLite 存储实现（T2 实现）

use sqlx::sqlite::SqlitePool;

/// SQLite 存储后端
pub struct SqliteStore {
    pool: SqlitePool,
}

impl SqliteStore {
    /// 创建新的 SQLite 存储，连接到指定数据库文件
    pub async fn new(db_path: &str) -> Result<Self, sqlx::Error> {
        let url = format!("sqlite:{}?mode=rwc", db_path);
        let pool = SqlitePool::connect(&url).await?;
        Ok(Self { pool })
    }

    /// 运行数据库迁移
    pub async fn migrate(&self) -> Result<(), sqlx::Error> {
        sqlx::query(MIGRATION_SQL).execute(&self.pool).await?;
        Ok(())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

const MIGRATION_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS providers (
    id           TEXT PRIMARY KEY,
    vendor       TEXT NOT NULL,
    display_name TEXT NOT NULL,
    api_key      TEXT NOT NULL,
    base_url     TEXT NOT NULL,
    enabled      INTEGER NOT NULL DEFAULT 1,
    created_at   TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS models (
    provider_id            TEXT NOT NULL,
    vendor_model_name      TEXT NOT NULL,
    display_name           TEXT NOT NULL,
    aliases                TEXT NOT NULL DEFAULT '[]',
    model_type             TEXT NOT NULL DEFAULT 'chat',

    supports_streaming     INTEGER NOT NULL DEFAULT 1,
    supports_tools         INTEGER NOT NULL DEFAULT 0,
    supports_structured_output INTEGER NOT NULL DEFAULT 0,
    supports_vision        INTEGER NOT NULL DEFAULT 0,
    supports_prefill       INTEGER NOT NULL DEFAULT 0,
    supports_cache         INTEGER NOT NULL DEFAULT 0,
    supports_web_search    INTEGER NOT NULL DEFAULT 0,
    supports_batch         INTEGER NOT NULL DEFAULT 0,
    context_window         INTEGER NOT NULL DEFAULT 0,

    cache_enabled          INTEGER NOT NULL DEFAULT 1,

    input_multiplier       REAL NOT NULL DEFAULT 1.0,
    output_multiplier      REAL NOT NULL DEFAULT 1.0,
    cache_write_multiplier REAL,
    cache_read_multiplier  REAL,

    image_input_multiplier       REAL,
    audio_input_multiplier       REAL,
    video_input_multiplier       REAL,
    image_generation_multiplier  REAL,
    tts_multiplier               REAL,

    extra_headers          TEXT NOT NULL DEFAULT '{}',

    enabled                INTEGER NOT NULL DEFAULT 1,
    created_at             TEXT NOT NULL,
    PRIMARY KEY (provider_id, vendor_model_name)
);

CREATE TABLE IF NOT EXISTS api_keys (
    id               TEXT PRIMARY KEY,
    key              TEXT NOT NULL UNIQUE,
    name             TEXT NOT NULL,
    model_group      TEXT NOT NULL DEFAULT '[]',
    default_model    TEXT NOT NULL DEFAULT '',
    fallback_models  TEXT NOT NULL DEFAULT '[]',
    credit_limit     INTEGER NOT NULL DEFAULT 0,
    credit_used      INTEGER NOT NULL DEFAULT 0,
    enabled          INTEGER NOT NULL DEFAULT 1,
    created_at       TEXT NOT NULL,
    last_used_at     TEXT
);

CREATE TABLE IF NOT EXISTS usage_records (
    id                TEXT PRIMARY KEY,
    api_key_id        TEXT NOT NULL,
    provider_id       TEXT NOT NULL,
    vendor_model_name TEXT NOT NULL,

    input_tokens         INTEGER NOT NULL DEFAULT 0,
    cache_write_tokens   INTEGER NOT NULL DEFAULT 0,
    cache_read_tokens    INTEGER NOT NULL DEFAULT 0,
    output_tokens        INTEGER NOT NULL DEFAULT 0,

    image_input_units    INTEGER NOT NULL DEFAULT 0,
    audio_input_seconds  REAL NOT NULL DEFAULT 0,
    video_input_seconds  REAL NOT NULL DEFAULT 0,
    image_gen_units      INTEGER NOT NULL DEFAULT 0,
    tts_characters       INTEGER NOT NULL DEFAULT 0,

    credits_consumed     INTEGER NOT NULL DEFAULT 0,

    created_at           TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_usage_api_key ON usage_records(api_key_id);
CREATE INDEX IF NOT EXISTS idx_usage_model ON usage_records(provider_id, vendor_model_name);
CREATE INDEX IF NOT EXISTS idx_usage_created ON usage_records(created_at);
"#;
