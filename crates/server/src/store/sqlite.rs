//! SQLite 存储实现 — KV + JSON 模式
//!
//! 使用单表 entities 存储所有实体，核心数据以 JSON 存储在 data 字段中。
//! 结构体字段变更不需要 migration，通过 serde(default) 自动兼容。

use async_trait::async_trait;
use sqlx::sqlite::{SqlitePool, SqliteRow};
use sqlx::Row;

use crate::models::api_key::ApiKey;
use crate::models::model::Model;
use crate::models::provider::Provider;
use crate::models::usage::UsageRecord;
use crate::store::error::StoreError;
use crate::store::traits::{ProxyStore, UsageQuery, UsageSummary};

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

// --- 辅助函数 ---

fn parse_json<T: serde::de::DeserializeOwned>(row: &SqliteRow) -> Result<T, StoreError> {
    let data: String = row.get("data");
    serde_json::from_str(&data).map_err(|e| StoreError::Serialization(e.to_string()))
}

// --- ProxyStore 实现 ---

#[async_trait]
impl ProxyStore for SqliteStore {
    // --- Provider ---

    async fn get_provider(&self, id: &str) -> Result<Option<Provider>, StoreError> {
        let row = sqlx::query("SELECT data FROM entities WHERE kind = 'provider' AND id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(parse_json::<Provider>).transpose()
    }

    async fn list_providers(&self) -> Result<Vec<Provider>, StoreError> {
        let rows = sqlx::query(
            "SELECT data FROM entities WHERE kind = 'provider' ORDER BY created_at"
        )
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(parse_json::<Provider>).collect()
    }

    async fn upsert_provider(&self, p: &Provider) -> Result<(), StoreError> {
        let data = serde_json::to_string(p)?;
        sqlx::query(
            "INSERT INTO entities (kind, id, secondary_key, enabled, data, created_at)
             VALUES ('provider', ?, NULL, ?, ?, ?)
             ON CONFLICT(kind, id) DO UPDATE SET
                data = excluded.data,
                enabled = excluded.enabled"
        )
        .bind(&p.id)
        .bind(p.enabled as i32)
        .bind(&data)
        .bind(p.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn delete_provider(&self, id: &str) -> Result<(), StoreError> {
        sqlx::query("DELETE FROM entities WHERE kind = 'provider' AND id = ?")
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
        let id = format!("{}/{}", provider_id, vendor_model_name);
        let row = sqlx::query("SELECT data FROM entities WHERE kind = 'model' AND id = ?")
            .bind(&id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(parse_json::<Model>).transpose()
    }

    async fn list_models(&self) -> Result<Vec<Model>, StoreError> {
        let rows = sqlx::query(
            "SELECT data FROM entities WHERE kind = 'model' ORDER BY created_at"
        )
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(parse_json::<Model>).collect()
    }

    async fn list_models_by_provider(
        &self,
        provider_id: &str,
    ) -> Result<Vec<Model>, StoreError> {
        // Use LIKE prefix match on id (which is "provider_id/model_name")
        let prefix = format!("{}/", provider_id);
        let rows = sqlx::query(
            "SELECT data FROM entities WHERE kind = 'model' AND id LIKE ? ORDER BY created_at"
        )
        .bind(format!("{}%", prefix))
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(parse_json::<Model>).collect()
    }

    async fn find_model(&self, name: &str) -> Result<Option<Model>, StoreError> {
        // Try exact match on id first
        let row = sqlx::query(
            "SELECT data FROM entities WHERE kind = 'model' AND id = ? AND enabled = 1"
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;
        if let Some(ref r) = row {
            return Ok(Some(parse_json::<Model>(r)?));
        }

        // Fall back to alias search (need to scan all models)
        let rows = sqlx::query(
            "SELECT data FROM entities WHERE kind = 'model' AND enabled = 1"
        )
        .fetch_all(&self.pool)
        .await?;
        for r in &rows {
            let model: Model = parse_json(r)?;
            if model.matches(name) {
                return Ok(Some(model));
            }
        }
        Ok(None)
    }

    async fn upsert_model(&self, m: &Model) -> Result<(), StoreError> {
        let id = m.id();
        let data = serde_json::to_string(m)?;
        sqlx::query(
            "INSERT INTO entities (kind, id, secondary_key, enabled, data, created_at)
             VALUES ('model', ?, NULL, ?, ?, ?)
             ON CONFLICT(kind, id) DO UPDATE SET
                data = excluded.data,
                enabled = excluded.enabled"
        )
        .bind(&id)
        .bind(m.enabled as i32)
        .bind(&data)
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
        let id = format!("{}/{}", provider_id, vendor_model_name);
        sqlx::query("DELETE FROM entities WHERE kind = 'model' AND id = ?")
            .bind(&id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // --- ApiKey ---

    async fn get_api_key_by_key(&self, key: &str) -> Result<Option<ApiKey>, StoreError> {
        let row = sqlx::query(
            "SELECT data FROM entities WHERE kind = 'api_key' AND secondary_key = ?"
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(parse_json::<ApiKey>).transpose()
    }

    async fn get_api_key_by_id(&self, id: &str) -> Result<Option<ApiKey>, StoreError> {
        let row = sqlx::query("SELECT data FROM entities WHERE kind = 'api_key' AND id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(parse_json::<ApiKey>).transpose()
    }

    async fn list_api_keys(&self) -> Result<Vec<ApiKey>, StoreError> {
        let rows = sqlx::query(
            "SELECT data FROM entities WHERE kind = 'api_key' ORDER BY created_at"
        )
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(parse_json::<ApiKey>).collect()
    }

    async fn upsert_api_key(&self, k: &ApiKey) -> Result<(), StoreError> {
        let data = serde_json::to_string(k)?;
        sqlx::query(
            "INSERT INTO entities (kind, id, secondary_key, enabled, data, created_at)
             VALUES ('api_key', ?, ?, ?, ?, ?)
             ON CONFLICT(kind, id) DO UPDATE SET
                data = excluded.data,
                secondary_key = excluded.secondary_key,
                enabled = excluded.enabled"
        )
        .bind(&k.id)
        .bind(&k.key)
        .bind(k.enabled as i32)
        .bind(&data)
        .bind(k.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn delete_api_key(&self, id: &str) -> Result<(), StoreError> {
        sqlx::query("DELETE FROM entities WHERE kind = 'api_key' AND id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn add_credits_used(&self, key_id: &str, credits: i64) -> Result<(), StoreError> {
        // Read, modify, write — atomic via single connection
        let row = sqlx::query("SELECT data FROM entities WHERE kind = 'api_key' AND id = ?")
            .bind(key_id)
            .fetch_optional(&self.pool)
            .await?;
        if let Some(ref r) = row {
            let mut key: ApiKey = parse_json(r)?;
            key.credit_used += credits;
            let data = serde_json::to_string(&key)?;
            sqlx::query("UPDATE entities SET data = ? WHERE kind = 'api_key' AND id = ?")
                .bind(&data)
                .bind(key_id)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    async fn reset_credits(&self, key_id: &str) -> Result<(), StoreError> {
        let row = sqlx::query("SELECT data FROM entities WHERE kind = 'api_key' AND id = ?")
            .bind(key_id)
            .fetch_optional(&self.pool)
            .await?;
        if let Some(ref r) = row {
            let mut key: ApiKey = parse_json(r)?;
            key.credit_used = 0;
            let data = serde_json::to_string(&key)?;
            sqlx::query("UPDATE entities SET data = ? WHERE kind = 'api_key' AND id = ?")
                .bind(&data)
                .bind(key_id)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    // --- Usage ---

    async fn insert_usage(&self, record: &UsageRecord) -> Result<(), StoreError> {
        let data = serde_json::to_string(record)?;
        sqlx::query(
            "INSERT INTO entities (kind, id, secondary_key, enabled, data, created_at)
             VALUES ('usage', ?, ?, 1, ?, ?)"
        )
        .bind(&record.id)
        .bind(&record.api_key_id) // secondary_key = api_key_id for fast lookup
        .bind(&data)
        .bind(record.created_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn query_usage(&self, query: &UsageQuery) -> Result<Vec<UsageRecord>, StoreError> {
        // Load all usage records and filter in memory
        // (acceptable for small-medium datasets; for large scale, add SQL WHERE clauses)
        let rows = sqlx::query(
            "SELECT data FROM entities WHERE kind = 'usage' ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        let limit = query.limit.unwrap_or(1000);
        let mut results = Vec::new();

        for row in &rows {
            let record: UsageRecord = parse_json(row)?;

            if let Some(ref key_id) = query.api_key_id {
                if record.api_key_id != *key_id {
                    continue;
                }
            }
            if let Some(ref pid) = query.provider_id {
                if record.provider_id != *pid {
                    continue;
                }
            }
            if let Some(ref mname) = query.vendor_model_name {
                if record.vendor_model_name != *mname {
                    continue;
                }
            }
            if let Some(ref start) = query.start_time {
                if record.created_at < *start {
                    continue;
                }
            }
            if let Some(ref end) = query.end_time {
                if record.created_at > *end {
                    continue;
                }
            }

            results.push(record);
            if results.len() >= limit {
                break;
            }
        }

        Ok(results)
    }

    async fn summarize_usage(&self, query: &UsageQuery) -> Result<UsageSummary, StoreError> {
        let records = self.query_usage(&UsageQuery {
            limit: None, // no limit for summary
            ..query.clone()
        }).await?;

        let mut summary = UsageSummary {
            total_requests: 0,
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cache_write_tokens: 0,
            total_cache_read_tokens: 0,
            total_credits: 0,
        };

        for r in &records {
            summary.total_requests += 1;
            summary.total_input_tokens += r.input_tokens as u64;
            summary.total_output_tokens += r.output_tokens as u64;
            summary.total_cache_write_tokens += r.cache_write_tokens as u64;
            summary.total_cache_read_tokens += r.cache_read_tokens as u64;
            summary.total_credits += r.credits_consumed;
        }

        Ok(summary)
    }
}

// --- Migration SQL ---

const MIGRATION_STATEMENTS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS entities (
        kind          TEXT NOT NULL,
        id            TEXT NOT NULL,
        secondary_key TEXT,
        enabled       INTEGER NOT NULL DEFAULT 1,
        data          TEXT NOT NULL,
        created_at    TEXT NOT NULL,
        PRIMARY KEY (kind, id)
    )",
    "CREATE INDEX IF NOT EXISTS idx_secondary ON entities(kind, secondary_key)",
    "CREATE INDEX IF NOT EXISTS idx_kind_enabled ON entities(kind, enabled)",
];

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::model::{ApiFormat, ModelType};
    use crate::models::provider::Vendor;
    use crate::store::ProxyStore;
    use chrono::Utc;
    use std::collections::HashMap;

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
            api_formats: vec![ApiFormat::OpenAI],
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

        store.upsert_provider(&p).await.unwrap();

        let got = store.get_provider("openai-main").await.unwrap().unwrap();
        assert_eq!(got.id, "openai-main");
        assert_eq!(got.api_key, "sk-test-123");

        let all = store.list_providers().await.unwrap();
        assert_eq!(all.len(), 1);

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

    #[tokio::test]
    async fn model_with_api_formats() {
        let store = test_store().await;
        let mut m = test_model("openai", "gpt-4o");
        m.api_formats = vec![ApiFormat::OpenAI, ApiFormat::Anthropic];
        store.upsert_model(&m).await.unwrap();

        let got = store.get_model("openai", "gpt-4o").await.unwrap().unwrap();
        assert_eq!(got.api_formats.len(), 2);
        assert!(got.api_formats.contains(&ApiFormat::OpenAI));
        assert!(got.api_formats.contains(&ApiFormat::Anthropic));
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

        store.add_credits_used("key1", 500).await.unwrap();
        store.add_credits_used("key1", 300).await.unwrap();

        let got = store.get_api_key_by_id("key1").await.unwrap().unwrap();
        assert_eq!(got.credit_used, 800);

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
        store.migrate().await.unwrap();
    }
}
