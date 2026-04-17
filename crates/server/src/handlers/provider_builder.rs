//! Provider 构建 — 根据 Model 的 api_formats 选择合适的 Provider 实现

use std::collections::HashMap;
use std::sync::Arc;

use lortex_core::provider::Provider;
use lortex_providers::CacheStrategy;

use crate::models::model::ApiFormat;
use crate::models::Model;
use crate::models::provider::Provider as ProviderConfig;

/// 合并 model extra_headers 与客户端 headers（客户端优先）
///
/// 对于 `anthropic-beta` 等逗号分隔的 header，合并值并去重。
pub(crate) fn merge_headers(
    model_headers: &HashMap<String, String>,
    client_headers: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut merged = model_headers.clone();

    for (key, client_val) in client_headers {
        let lower_key = key.to_lowercase();
        // anthropic-beta 等逗号分隔的 header 需要合并值
        if lower_key == "anthropic-beta" {
            if let Some(model_val) = merged.get(&lower_key) {
                let mut parts: Vec<&str> = model_val.split(',').map(|s| s.trim()).collect();
                for part in client_val.split(',').map(|s| s.trim()) {
                    if !parts.contains(&part) {
                        parts.push(part);
                    }
                }
                merged.insert(lower_key, parts.join(", "));
            } else {
                merged.insert(lower_key, client_val.clone());
            }
        } else {
            // 客户端优先覆盖
            merged.insert(lower_key, client_val.clone());
        }
    }

    merged
}

/// 根据请求来源格式和模型支持的 api_formats，选择最合适的 Provider 实现
///
/// `client_headers`: 从客户端请求中提取的、需要透传给上游的 header
pub fn build_llm_provider(
    provider_config: &ProviderConfig,
    model: &Model,
    preferred_format: &ApiFormat,
    client_headers: &HashMap<String, String>,
) -> Arc<dyn Provider> {
    let headers = merge_headers(&model.extra_headers, client_headers);

    // 读取缓存注入策略
    let cache_strategy = if model.cache_enabled {
        CacheStrategy::from_str(&model.cache_strategy)
    } else {
        CacheStrategy::None
    };

    // 选择实际使用的 API 格式
    let actual_format = if model.api_formats.contains(preferred_format) {
        preferred_format.clone()
    } else if let Some(first) = model.api_formats.first() {
        first.clone()
    } else {
        ApiFormat::OpenAI
    };

    let base = format!("{}/v1", provider_config.base_url.trim_end_matches('/'));

    match actual_format {
        ApiFormat::OpenAI => {
            Arc::new(
                lortex_providers::openai::OpenAIProvider::new(&provider_config.api_key)
                    .with_base_url(&base)
                    .with_extra_headers(headers)
                    .with_cache_strategy(cache_strategy),
            )
        }
        ApiFormat::Anthropic => {
            Arc::new(
                lortex_providers::anthropic::AnthropicProvider::new(&provider_config.api_key)
                    .with_base_url(&base)
                    .with_extra_headers(headers)
                    .with_cache_strategy(cache_strategy),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_headers_client_overrides() {
        let mut model = HashMap::new();
        model.insert("x-custom".into(), "model-val".into());
        let mut client = HashMap::new();
        client.insert("x-custom".into(), "client-val".into());

        let merged = merge_headers(&model, &client);
        assert_eq!(merged.get("x-custom").unwrap(), "client-val");
    }

    #[test]
    fn merge_headers_anthropic_beta_combines() {
        let mut model = HashMap::new();
        model.insert("anthropic-beta".into(), "feature-a".into());
        let mut client = HashMap::new();
        client.insert("anthropic-beta".into(), "feature-b".into());

        let merged = merge_headers(&model, &client);
        let val = merged.get("anthropic-beta").unwrap();
        assert!(val.contains("feature-a"));
        assert!(val.contains("feature-b"));
    }

    #[test]
    fn merge_headers_anthropic_beta_deduplicates() {
        let mut model = HashMap::new();
        model.insert("anthropic-beta".into(), "feature-a, feature-b".into());
        let mut client = HashMap::new();
        client.insert("anthropic-beta".into(), "feature-b, feature-c".into());

        let merged = merge_headers(&model, &client);
        let val = merged.get("anthropic-beta").unwrap();
        let parts: Vec<&str> = val.split(',').map(|s| s.trim()).collect();
        assert_eq!(parts.len(), 3); // a, b, c — no dups
    }

    #[test]
    fn merge_headers_empty_client() {
        let mut model = HashMap::new();
        model.insert("x-key".into(), "val".into());
        let client = HashMap::new();

        let merged = merge_headers(&model, &client);
        assert_eq!(merged.get("x-key").unwrap(), "val");
    }
}
