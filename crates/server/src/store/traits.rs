//! ProxyStore trait — 可插拔存储接口

use async_trait::async_trait;

use crate::models::{ApiKey, Model, ModelHealthStatus, Provider, UsageRecord};
use crate::store::StoreError;

/// 用量查询参数
#[derive(Debug, Clone, Default)]
pub struct UsageQuery {
    pub api_key_id: Option<String>,
    pub provider_id: Option<String>,
    pub vendor_model_name: Option<String>,
    pub start_time: Option<chrono::DateTime<chrono::Utc>>,
    pub end_time: Option<chrono::DateTime<chrono::Utc>>,
    pub limit: Option<usize>,
}

/// 用量汇总
#[derive(Debug, Clone, serde::Serialize)]
pub struct UsageSummary {
    pub total_requests: u64,
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cache_write_tokens: u64,
    pub total_cache_read_tokens: u64,
}

/// 时间趋势数据点（按日分桶）
#[derive(Debug, Clone, serde::Serialize)]
pub struct TrendPoint {
    /// 分桶起始时间（ISO 8601 日期，如 "2026-04-13"）
    pub date: String,
    pub requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_read_tokens: u64,
}

/// 分组聚合结果（用于 by-model / by-key）
#[derive(Debug, Clone, serde::Serialize)]
pub struct GroupedUsage {
    /// 分组键（model_id 如 "openai/gpt-4o"，或 api_key_id）
    pub group_key: String,
    /// 可选显示名（api_key_name 或 model display_name）
    pub display_name: String,
    pub requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_write_tokens: u64,
    pub cache_read_tokens: u64,
}

#[async_trait]
pub trait ProxyStore: Send + Sync {
    // --- Provider ---
    async fn get_provider(&self, id: &str) -> Result<Option<Provider>, StoreError>;
    async fn list_providers(&self) -> Result<Vec<Provider>, StoreError>;
    async fn upsert_provider(&self, p: &Provider) -> Result<(), StoreError>;
    async fn delete_provider(&self, id: &str) -> Result<(), StoreError>;

    // --- Model ---
    async fn get_model(
        &self,
        provider_id: &str,
        vendor_model_name: &str,
    ) -> Result<Option<Model>, StoreError>;
    async fn list_models(&self) -> Result<Vec<Model>, StoreError>;
    async fn list_models_by_provider(
        &self,
        provider_id: &str,
    ) -> Result<Vec<Model>, StoreError>;
    async fn find_model(&self, name: &str) -> Result<Option<Model>, StoreError>;
    async fn upsert_model(&self, m: &Model) -> Result<(), StoreError>;
    async fn delete_model(
        &self,
        provider_id: &str,
        vendor_model_name: &str,
    ) -> Result<(), StoreError>;

    // --- ApiKey ---
    async fn get_api_key_by_key(&self, key: &str) -> Result<Option<ApiKey>, StoreError>;
    async fn get_api_key_by_id(&self, id: &str) -> Result<Option<ApiKey>, StoreError>;
    async fn list_api_keys(&self) -> Result<Vec<ApiKey>, StoreError>;
    async fn upsert_api_key(&self, k: &ApiKey) -> Result<(), StoreError>;
    async fn delete_api_key(&self, id: &str) -> Result<(), StoreError>;

    // --- Usage ---
    async fn insert_usage(&self, record: &UsageRecord) -> Result<(), StoreError>;
    async fn query_usage(&self, query: &UsageQuery) -> Result<Vec<UsageRecord>, StoreError>;
    async fn summarize_usage(&self, query: &UsageQuery) -> Result<UsageSummary, StoreError>;
    async fn usage_trend(&self, query: &UsageQuery) -> Result<Vec<TrendPoint>, StoreError>;
    async fn usage_by_model(&self, query: &UsageQuery) -> Result<Vec<GroupedUsage>, StoreError>;
    async fn usage_by_key(&self, query: &UsageQuery) -> Result<Vec<GroupedUsage>, StoreError>;

    // --- Health ---
    async fn get_health_status(
        &self,
        model_id: &str,
    ) -> Result<Option<ModelHealthStatus>, StoreError>;
    async fn list_health_statuses(&self) -> Result<Vec<ModelHealthStatus>, StoreError>;
    async fn upsert_health_status(
        &self,
        status: &ModelHealthStatus,
    ) -> Result<(), StoreError>;
}
