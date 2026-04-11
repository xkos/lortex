//! Provider 构建 — 根据 Model 的 api_formats 选择合适的 Provider 实现

use std::sync::Arc;

use lortex_core::provider::Provider;

use crate::models::model::ApiFormat;
use crate::models::Model;
use crate::models::provider::Provider as ProviderConfig;

/// 根据请求来源格式和模型支持的 api_formats，选择最合适的 Provider 实现
///
/// 逻辑：
/// - 如果模型支持请求方的格式，直接用该格式（无需转换）
/// - 否则用模型支持的第一个格式（proxy 层做协议转换）
pub fn build_llm_provider(
    provider_config: &ProviderConfig,
    model: &Model,
    preferred_format: &ApiFormat,
) -> Arc<dyn Provider> {
    // 选择实际使用的 API 格式
    let actual_format = if model.api_formats.contains(preferred_format) {
        preferred_format.clone()
    } else if let Some(first) = model.api_formats.first() {
        first.clone()
    } else {
        // 默认 OpenAI
        ApiFormat::OpenAI
    };

    match actual_format {
        ApiFormat::OpenAI => {
            Arc::new(
                lortex_providers::openai::OpenAIProvider::new(&provider_config.api_key)
                    .with_base_url(&provider_config.base_url)
                    .with_extra_headers(model.extra_headers.clone()),
            )
        }
        ApiFormat::Anthropic => {
            Arc::new(
                lortex_providers::anthropic::AnthropicProvider::new(&provider_config.api_key)
                    .with_base_url(&provider_config.base_url)
                    .with_extra_headers(model.extra_headers.clone()),
            )
        }
    }
}
