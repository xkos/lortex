//! SQLite 存储实现

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::sqlite::{SqlitePool, SqliteRow};
use sqlx::Row;

use crate::models::api_key::ApiKey;
use crate::models::model::{Model, ModelType};
use crate::models::provider::{Provider, Vendor};
use crate::store::error::StoreError;
use crate::store::traits::ProxyStore;

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
        for statement in MIGRATION_STATEMENTS {
            sqlx::query(statement).execute(&self.pool).await?;
        }
        Ok(())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

// --- Row → Model 转换辅助 ---

fn row_to_provider(row: &SqliteRow) -> Result<Provider, StoreError> {
    Ok(Provider {
        id: row.get("id"),
        vendor: Vendor::from_str(row.get("vendor")),
        display_name: row.get("display_name"),
        api_key: row.get("api_key"),
        base_url: row.get("base_url"),
        enabled: row.get::<i32, _>("enabled") != 0,
        created_at: row.get::<String, _>("created_at").parse::<DateTime<Utc>>()
            .unwrap_or_else(|_| Utc::now()),
    })
}

fn row_to_model(row: &SqliteRow) -> Result<Model, StoreError> {
    let aliases_json: String = row.get("aliases");
    let aliases: Vec<String> = serde_json::from_str(&aliases_json).unwrap_or_default();
    let headers_json: String = row.get("extra_headers");
    let extra_headers: HashMap<String, String> = serde_json::from_str(&headers_json).unwrap_or_default();

    Ok(Model {
        provider_id: row.get("provider_id"),
        vendor_model_name: row.get("vendor_model_name"),
        display_name: row.get("display_name"),
        aliases,
        model_type: ModelType::from_str(row.get("model_type")),
        supports_streaming: row.get::<i32, _>("supports_streaming") != 0,
        supports_tools: row.get::<i32, _>("supports_tools") != 0,
        supports_structured_output: row.get::<i32, _>("supports_structured_output") != 0,
        supports_vision: row.get::<i32, _>("supports_vision") != 0,
        supports_prefill: row.get::<i32, _>("supports_prefill") != 0,
        supports_cache: row.get::<i32, _>("supports_cache") != 0,
        supports_web_search: row.get::<i32, _>("supports_web_search") != 0,
        supports_batch: row.get::<i32, _>("supports_batch") != 0,
        context_window: row.get::<i32, _>("context_window") as u32,
        cache_enabled: row.get::<i32, _>("cache_enabled") != 0,
        input_multiplier: row.get("input_multiplier"),
        output_multiplier: row.get("output_multiplier"),
        cache_write_multiplier: row.get("cache_write_multiplier"),
        cache_read_multiplier: row.get("cache_read_multiplier"),
        image_input_multiplier: row.get("image_input_multiplier"),
        audio_input_multiplier: row.get("audio_input_multiplier"),
        video_input_multiplier: row.get("video_input_multiplier"),
        image_generation_multiplier: row.get("image_generation_multiplier"),
        tts_multiplier: row.get("tts_multiplier"),
        extra_headers,
        enabled: row.get::<i32, _>("enabled") != 0,
        created_at: row.get::<String, _>("created_at").parse::<DateTime<Utc>>()
            .unwrap_or_else(|_| Utc::now()),
    })
}

fn row_to_api_key(row: &SqliteRow) -> Result<ApiKey, StoreError> {
    let model_group_json: String = row.get("model_group");
    let model_group: Vec<String> = serde_json::from_str(&model_group_json).unwrap_or_default();
    let fallback_json: String = row.get("fallback_models");
    let fallback_models: Vec<String> = serde_json::from_str(&fallback_json).unwrap_or_default();
    let last_used: Option<String> = row.get("last_used_at");

    Ok(ApiKey {
        id: row.get("id"),
        key: row.get("key"),
        name: row.get("name"),
        model_group,
        default_model: row.get("default_model"),
        fallback_models,
        credit_limit: row.get::<i64, _>("credit_limit"),
        credit_used: row.get::<i64, _>("credit_used"),
        enabled: row.get::<i32, _>("enabled") != 0,
        created_at: row.get::<String, _>("created_at").parse::<DateTime<Utc>>()
            .unwrap_or_else(|_| Utc::now()),
        last_used_at: last_used.and_then(|s| s.parse::<DateTime<Utc>>().ok()),
    })
}

// --- ProxyStore 实现 ---

#[async_trait]
impl ProxyStore for SqliteStore {
    // --- Provider ---

    async fn get_provider(&self, id: &str) -> Result<Option<Provider>, StoreError> {
        let row = sqlx::query("SELECT * FROM providers WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(row_to_provider).transpose()
    }

    async fn list_providers(&self) -> Result<Vec<Provider>, StoreError> {
        let rows = sqlx::query("SELECT * FROM providers ORDER BY created_at")
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(row_to_provider).collect()
    }

    async fn upsert_provider(&self, p: &Provider) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO providers (id, vendor, display_name, api_key, base_url, enabled, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                vendor = excluded.vendor,
                display_name = excluded.display_name,
                api_key = excluded.api_key,
                base_url = excluded.base_url,
                enabled = excluded.enabled"
        )
        .bind(&p.id)
        .bind(p.vendor.as_str())
        .bind(&p.display_name)
        .bind(&p.api_key)
        .bind(&p.base_url)
        .bind(p.enabled as i32)
        .bind(p.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn delete_provider(&self, id: &str) -> Result<(), StoreError> {
        sqlx::query("DELETE FROM providers WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // --- Model ---

    async fn get_model(
        &self,
        provider_id: &str,
        vendor_model_name: &str,
    ) -> Result<Option<Model>, StoreError> {
        let row = sqlx::query(
            "SELECT * FROM models WHERE provider_id = ? AND vendor_model_name = ?"
        )
        .bind(provider_id)
        .bind(vendor_model_name)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(row_to_model).transpose()
    }

    async fn list_models(&self) -> Result<Vec<Model>, StoreError> {
        let rows = sqlx::query("SELECT * FROM models ORDER BY created_at")
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(row_to_model).collect()
    }

    async fn list_models_by_provider(
        &self,
        provider_id: &str,
    ) -> Result<Vec<Model>, StoreError> {
        let rows = sqlx::query("SELECT * FROM models WHERE provider_id = ? ORDER BY created_at")
            .bind(provider_id)
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(row_to_model).collect()
    }

    async fn find_model(&self, name: &str) -> Result<Option<Model>, StoreError> {
        // Try exact match on "provider_id/vendor_model_name" first
        if let Some((pid, mname)) = name.split_once('/') {
            if let Some(m) = self.get_model(pid, mname).await? {
                return Ok(Some(m));
            }
        }
        // Fall back to alias search
        let rows = sqlx::query("SELECT * FROM models WHERE enabled = 1")
            .fetch_all(&self.pool)
            .await?;
        for row in &rows {
            let model = row_to_model(row)?;
            if model.matches(name) {
                return Ok(Some(model));
            }
        }
        Ok(None)
    }

    async fn upsert_model(&self, m: &Model) -> Result<(), StoreError> {
        let aliases_json = serde_json::to_string(&m.aliases)?;
        let headers_json = serde_json::to_string(&m.extra_headers)?;

        sqlx::query(
            "INSERT INTO models (
                provider_id, vendor_model_name, display_name, aliases, model_type,
                supports_streaming, supports_tools, supports_structured_output,
                supports_vision, supports_prefill, supports_cache,
                supports_web_search, supports_batch, context_window,
                cache_enabled,
                input_multiplier, output_multiplier,
                cache_write_multiplier, cache_read_multiplier,
                image_input_multiplier, audio_input_multiplier,
                video_input_multiplier, image_generation_multiplier, tts_multiplier,
                extra_headers, enabled, created_at
             ) VALUES (
                ?, ?, ?, ?, ?,
                ?, ?, ?,
                ?, ?, ?,
                ?, ?, ?,
                ?,
                ?, ?,
                ?, ?,
                ?, ?,
                ?, ?, ?,
                ?, ?, ?
             )
             ON CONFLICT(provider_id, vendor_model_name) DO UPDATE SET
                display_name = excluded.display_name,
                aliases = excluded.aliases,
                model_type = excluded.model_type,
                supports_streaming = excluded.supports_streaming,
                supports_tools = excluded.supports_tools,
                supports_structured_output = excluded.supports_structured_output,
                supports_vision = excluded.supports_vision,
                supports_prefill = excluded.supports_prefill,
                supports_cache = excluded.supports_cache,
                supports_web_search = excluded.supports_web_search,
                supports_batch = excluded.supports_batch,
                context_window = excluded.context_window,
                cache_enabled = excluded.cache_enabled,
                input_multiplier = excluded.input_multiplier,
                output_multiplier = excluded.output_multiplier,
                cache_write_multiplier = excluded.cache_write_multiplier,
                cache_read_multiplier = excluded.cache_read_multiplier,
                image_input_multiplier = excluded.image_input_multiplier,
                audio_input_multiplier = excluded.audio_input_multiplier,
                video_input_multiplier = excluded.video_input_multiplier,
                image_generation_multiplier = excluded.image_generation_multiplier,
                tts_multiplier = excluded.tts_multiplier,
                extra_headers = excluded.extra_headers,
                enabled = excluded.enabled"
        )
        .bind(&m.provider_id)
        .bind(&m.vendor_model_name)
        .bind(&m.display_name)
        .bind(&aliases_json)
        .bind(m.model_type.as_str())
        .bind(m.supports_streaming as i32)
        .bind(m.supports_tools as i32)
        .bind(m.supports_structured_output as i32)
        .bind(m.supports_vision as i32)
        .bind(m.supports_prefill as i32)
        .bind(m.supports_cache as i32)
        .bind(m.supports_web_search as i32)
        .bind(m.supports_batch as i32)
        .bind(m.context_window as i32)
        .bind(m.cache_enabled as i32)
        .bind(m.input_multiplier)
        .bind(m.output_multiplier)
        .bind(m.cache_write_multiplier)
        .bind(m.cache_read_multiplier)
        .bind(m.image_input_multiplier)
        .bind(m.audio_input_multiplier)
        .bind(m.video_input_multiplier)
        .bind(m.image_generation_multiplier)
        .bind(m.tts_multiplier)
        .bind(&headers_json)
        .bind(m.enabled as i32)
        .bind(m.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn delete_model(
        &self,
        provider_id: &str,
        vendor_model_name: &str,
    ) -> Result<(), StoreError> {
        sqlx::query("DELETE FROM models WHERE provider_id = ? AND vendor_model_name = ?")
            .bind(provider_id)
            .bind(vendor_model_name)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // --- ApiKey ---

    async fn get_api_key_by_key(&self, key: &str) -> Result<Option<ApiKey>, StoreError> {
        let row = sqlx::query("SELECT * FROM api_keys WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(row_to_api_key).transpose()
    }

    async fn get_api_key_by_id(&self, id: &str) -> Result<Option<ApiKey>, StoreError> {
        let row = sqlx::query("SELECT * FROM api_keys WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(row_to_api_key).transpose()
    }

    async fn list_api_keys(&self) -> Result<Vec<ApiKey>, StoreError> {
        let rows = sqlx::query("SELECT * FROM api_keys ORDER BY created_at")
            .fetch_all(&self.pool)
            .await?;
        rows.iter().map(row_to_api_key).collect()
    }

    async fn upsert_api_key(&self, k: &ApiKey) -> Result<(), StoreError> {
        let model_group_json = serde_json::to_string(&k.model_group)?;
        let fallback_json = serde_json::to_string(&k.fallback_models)?;
        let last_used = k.last_used_at.map(|t| t.to_rfc3339());

        sqlx::query(
            "INSERT INTO api_keys (
                id, key, name, model_group, default_model, fallback_models,
                credit_limit, credit_used, enabled, created_at, last_used_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
                key = excluded.key,
                name = excluded.name,
                model_group = excluded.model_group,
                default_model = excluded.default_model,
                fallback_models = excluded.fallback_models,
                credit_limit = excluded.credit_limit,
                credit_used = excluded.credit_used,
                enabled = excluded.enabled,
                last_used_at = excluded.last_used_at"
        )
        .bind(&k.id)
        .bind(&k.key)
        .bind(&k.name)
        .bind(&model_group_json)
        .bind(&k.default_model)
        .bind(&fallback_json)
        .bind(k.credit_limit)
        .bind(k.credit_used)
        .bind(k.enabled as i32)
        .bind(k.created_at.to_rfc3339())
        .bind(&last_used)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn delete_api_key(&self, id: &str) -> Result<(), StoreError> {
        sqlx::query("DELETE FROM api_keys WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn add_credits_used(&self, key_id: &str, credits: i64) -> Result<(), StoreError> {
        sqlx::query("UPDATE api_keys SET credit_used = credit_used + ? WHERE id = ?")
            .bind(credits)
            .bind(key_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn reset_credits(&self, key_id: &str) -> Result<(), StoreError> {
        sqlx::query("UPDATE api_keys SET credit_used = 0 WHERE id = ?")
            .bind(key_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

// --- Migration SQL ---

const MIGRATION_STATEMENTS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS providers (
        id           TEXT PRIMARY KEY,
        vendor       TEXT NOT NULL,
        display_name TEXT NOT NULL,
        api_key      TEXT NOT NULL,
        base_url     TEXT NOT NULL,
        enabled      INTEGER NOT NULL DEFAULT 1,
        created_at   TEXT NOT NULL
    )",
    "CREATE TABLE IF NOT EXISTS models (
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
    )",
    "CREATE TABLE IF NOT EXISTS api_keys (
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
    )",
    "CREATE TABLE IF NOT EXISTS usage_records (
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
    )",
    "CREATE INDEX IF NOT EXISTS idx_usage_api_key ON usage_records(api_key_id)",
    "CREATE INDEX IF NOT EXISTS idx_usage_model ON usage_records(provider_id, vendor_model_name)",
    "CREATE INDEX IF NOT EXISTS idx_usage_created ON usage_records(created_at)",
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::ProxyStore;

    async fn test_store() -> SqliteStore {
        let store = SqliteStore::new(":memory:").await.unwrap();
        store.migrate().await.unwrap();
        store
    }

    fn test_provider(id: &str) -> Provider {
        Provider {
            id: id.into(),
            vendor: Vendor::OpenAI,
            display_name: format!("{id} display"),
            api_key: "sk-test-123".into(),
            base_url: "https://api.openai.com".into(),
            enabled: true,
            created_at: Utc::now(),
        }
    }

    fn test_model(provider_id: &str, name: &str) -> Model {
        Model {
            provider_id: provider_id.into(),
            vendor_model_name: name.into(),
            display_name: format!("{name} display"),
            aliases: vec![],
            model_type: ModelType::Chat,
            supports_streaming: true,
            supports_tools: true,
            supports_structured_output: false,
            supports_vision: false,
            supports_prefill: false,
            supports_cache: false,
            supports_web_search: false,
            supports_batch: false,
            context_window: 128000,
            cache_enabled: true,
            input_multiplier: 2.5,
            output_multiplier: 10.0,
            cache_write_multiplier: None,
            cache_read_multiplier: None,
            image_input_multiplier: None,
            audio_input_multiplier: None,
            video_input_multiplier: None,
            image_generation_multiplier: None,
            tts_multiplier: None,
            extra_headers: HashMap::new(),
            enabled: true,
            created_at: Utc::now(),
        }
    }

    fn test_api_key(id: &str) -> ApiKey {
        ApiKey {
            id: id.into(),
            key: format!("sk-proxy-{id}"),
            name: format!("{id} key"),
            model_group: vec!["openai/gpt-4o".into()],
            default_model: "openai/gpt-4o".into(),
            fallback_models: vec![],
            credit_limit: 100000,
            credit_used: 0,
            enabled: true,
            created_at: Utc::now(),
            last_used_at: None,
        }
    }

    // --- Provider tests ---

    #[tokio::test]
    async fn provider_crud() {
        let store = test_store().await;
        let p = test_provider("openai-main");

        // Create
        store.upsert_provider(&p).await.unwrap();

        // Read
        let got = store.get_provider("openai-main").await.unwrap().unwrap();
        assert_eq!(got.id, "openai-main");
        assert_eq!(got.api_key, "sk-test-123");

        // List
        let all = store.list_providers().await.unwrap();
        assert_eq!(all.len(), 1);

        // Delete
        store.delete_provider("openai-main").await.unwrap();
        assert!(store.get_provider("openai-main").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn provider_upsert_updates() {
        let store = test_store().await;
        let mut p = test_provider("openai-main");
        store.upsert_provider(&p).await.unwrap();

        p.api_key = "sk-new-key".into();
        store.upsert_provider(&p).await.unwrap();

        let got = store.get_provider("openai-main").await.unwrap().unwrap();
        assert_eq!(got.api_key, "sk-new-key");
        assert_eq!(store.list_providers().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn provider_get_nonexistent() {
        let store = test_store().await;
        assert!(store.get_provider("ghost").await.unwrap().is_none());
    }

    // --- Model tests ---

    #[tokio::test]
    async fn model_crud() {
        let store = test_store().await;
        let m = test_model("openai", "gpt-4o");

        store.upsert_model(&m).await.unwrap();

        let got = store.get_model("openai", "gpt-4o").await.unwrap().unwrap();
        assert_eq!(got.display_name, "gpt-4o display");
        assert_eq!(got.input_multiplier, 2.5);

        let all = store.list_models().await.unwrap();
        assert_eq!(all.len(), 1);

        store.delete_model("openai", "gpt-4o").await.unwrap();
        assert!(store.get_model("openai", "gpt-4o").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn model_upsert_updates() {
        let store = test_store().await;
        let mut m = test_model("openai", "gpt-4o");
        store.upsert_model(&m).await.unwrap();

        m.input_multiplier = 5.0;
        m.supports_vision = true;
        store.upsert_model(&m).await.unwrap();

        let got = store.get_model("openai", "gpt-4o").await.unwrap().unwrap();
        assert_eq!(got.input_multiplier, 5.0);
        assert!(got.supports_vision);
        assert_eq!(store.list_models().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn model_list_by_provider() {
        let store = test_store().await;
        store.upsert_model(&test_model("openai", "gpt-4o")).await.unwrap();
        store.upsert_model(&test_model("openai", "gpt-4o-mini")).await.unwrap();
        store.upsert_model(&test_model("anthropic", "claude")).await.unwrap();

        let openai = store.list_models_by_provider("openai").await.unwrap();
        assert_eq!(openai.len(), 2);

        let anthropic = store.list_models_by_provider("anthropic").await.unwrap();
        assert_eq!(anthropic.len(), 1);
    }

    #[tokio::test]
    async fn model_find_by_id() {
        let store = test_store().await;
        store.upsert_model(&test_model("openai", "gpt-4o")).await.unwrap();

        let found = store.find_model("openai/gpt-4o").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().vendor_model_name, "gpt-4o");
    }

    #[tokio::test]
    async fn model_find_by_alias() {
        let store = test_store().await;
        let mut m = test_model("openai", "gpt-4o");
        m.aliases = vec!["gpt4".into(), "best-model".into()];
        store.upsert_model(&m).await.unwrap();

        let found = store.find_model("gpt4").await.unwrap();
        assert!(found.is_some());

        let found2 = store.find_model("best-model").await.unwrap();
        assert!(found2.is_some());

        let not_found = store.find_model("nonexistent").await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn model_with_optional_multipliers() {
        let store = test_store().await;
        let mut m = test_model("openai", "gpt-4o");
        m.cache_write_multiplier = Some(3.75);
        m.cache_read_multiplier = Some(0.3);
        m.image_input_multiplier = Some(1.5);
        store.upsert_model(&m).await.unwrap();

        let got = store.get_model("openai", "gpt-4o").await.unwrap().unwrap();
        assert_eq!(got.cache_write_multiplier, Some(3.75));
        assert_eq!(got.cache_read_multiplier, Some(0.3));
        assert_eq!(got.image_input_multiplier, Some(1.5));
        assert!(got.audio_input_multiplier.is_none());
    }

    #[tokio::test]
    async fn model_with_extra_headers() {
        let store = test_store().await;
        let mut m = test_model("openai", "gpt-4o");
        m.extra_headers.insert("X-Custom".into(), "value".into());
        store.upsert_model(&m).await.unwrap();

        let got = store.get_model("openai", "gpt-4o").await.unwrap().unwrap();
        assert_eq!(got.extra_headers.get("X-Custom").unwrap(), "value");
    }

    // --- ApiKey tests ---

    #[tokio::test]
    async fn api_key_crud() {
        let store = test_store().await;
        let k = test_api_key("key1");

        store.upsert_api_key(&k).await.unwrap();

        let by_id = store.get_api_key_by_id("key1").await.unwrap().unwrap();
        assert_eq!(by_id.name, "key1 key");

        let by_key = store.get_api_key_by_key("sk-proxy-key1").await.unwrap().unwrap();
        assert_eq!(by_key.id, "key1");

        let all = store.list_api_keys().await.unwrap();
        assert_eq!(all.len(), 1);

        store.delete_api_key("key1").await.unwrap();
        assert!(store.get_api_key_by_id("key1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn api_key_credits() {
        let store = test_store().await;
        let k = test_api_key("key1");
        store.upsert_api_key(&k).await.unwrap();

        // Add credits
        store.add_credits_used("key1", 500).await.unwrap();
        store.add_credits_used("key1", 300).await.unwrap();

        let got = store.get_api_key_by_id("key1").await.unwrap().unwrap();
        assert_eq!(got.credit_used, 800);

        // Reset
        store.reset_credits("key1").await.unwrap();
        let got = store.get_api_key_by_id("key1").await.unwrap().unwrap();
        assert_eq!(got.credit_used, 0);
    }

    #[tokio::test]
    async fn api_key_upsert_updates() {
        let store = test_store().await;
        let mut k = test_api_key("key1");
        store.upsert_api_key(&k).await.unwrap();

        k.name = "updated name".into();
        k.credit_limit = 999;
        store.upsert_api_key(&k).await.unwrap();

        let got = store.get_api_key_by_id("key1").await.unwrap().unwrap();
        assert_eq!(got.name, "updated name");
        assert_eq!(got.credit_limit, 999);
        assert_eq!(store.list_api_keys().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn api_key_model_group_roundtrip() {
        let store = test_store().await;
        let mut k = test_api_key("key1");
        k.model_group = vec![
            "openai/gpt-4o".into(),
            "anthropic/claude-sonnet".into(),
        ];
        k.fallback_models = vec!["openai/gpt-4o-mini".into()];
        store.upsert_api_key(&k).await.unwrap();

        let got = store.get_api_key_by_id("key1").await.unwrap().unwrap();
        assert_eq!(got.model_group.len(), 2);
        assert_eq!(got.fallback_models.len(), 1);
    }

    // --- Migration ---

    #[tokio::test]
    async fn migrate_is_idempotent() {
        let store = test_store().await;
        // Second migrate should not fail
        store.migrate().await.unwrap();
    }
}
