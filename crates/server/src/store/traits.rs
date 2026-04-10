//! ProxyStore trait — 可插拔存储接口

use async_trait::async_trait;

use crate::models::{ApiKey, Model, Provider};
use crate::store::StoreError;

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
    /// 按 ID（"provider_id/vendor_model_name"）或别名查找模型
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
    async fn add_credits_used(&self, key_id: &str, credits: i64) -> Result<(), StoreError>;
    async fn reset_credits(&self, key_id: &str) -> Result<(), StoreError>;
}
