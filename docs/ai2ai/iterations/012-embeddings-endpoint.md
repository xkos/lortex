# 迭代 012: /v1/embeddings 端点 — OpenAI 兼容 embedding 代理

> 分支：main（直接提交）
> 状态：✅ 完成

## 目标
补全 Proxy 的 embedding 端点，支持 OpenAI 兼容的 /v1/embeddings 请求代理。

## 完成内容

### 修改文件
| 文件 | 改动 |
|------|------|
| `crates/core/src/provider.rs` | 新增 EmbeddingRequest / EmbeddingResponse / EmbeddingData / EmbeddingUsage + embed 签名变更 + 4 tests |
| `crates/providers/src/openai.rs` | 实现 embed 方法（HTTP 调用 + 响应解析） |
| `crates/server/src/proto/openai.rs` | 新增 EmbeddingRequest / EmbeddingInput / EmbeddingResponse / EmbeddingObject / EmbeddingUsage + 4 tests |
| `crates/server/src/handlers/embed.rs` | 新建 — 完整 embedding handler（resolve + fallback + 限流 + usage） |
| `crates/server/src/handlers/mod.rs` | `pub mod embed;` |
| `crates/server/src/routes.rs` | `.route("/v1/embeddings", post(embed::embeddings))` |

### 架构
```
POST /v1/embeddings
  ↓ proxy_auth middleware（ApiKey 鉴权）
  ↓
embeddings handler
  ├── 解析 EmbeddingRequest（string | string[] input）
  ├── resolve_models_with_fallback（ModelType::Embedding 校验）
  ├── for model in candidates:
  │   ├── circuit_breaker 检查
  │   ├── model RPM/TPM 检查
  │   ├── build_provider_with_headers
  │   └── provider.embed() → EmbeddingResponse
  ├── tracing span → UsageLayer → credit 扣减 + usage 记录
  └── 返回 OpenAI 格式 EmbeddingResponse
```

## 关键设计决策
- **不经内部格式转换**：Chat 需要 OpenAI↔Anthropic 双格式互转所以有 lortex 内部类型，Embedding 只有 OpenAI 格式，直接透传更简单
- **EmbeddingData.embedding 用 serde_json::Value**：因为 encoding_format=base64 时返回 base64 字符串而非 float 数组，Value 统一处理两种格式
- **EmbeddingInput untagged enum**：支持 OpenAI API 的 `"input": "text"` 和 `"input": ["text1", "text2"]` 两种格式
- **output_tokens = 0**：Embedding API 无 completion tokens，显式记录 0 让 UsageLayer 正确处理

## 未完成/遗留
无

## 回归影响
- Provider trait embed 签名变更（从 `&[&str] -> Vec<Vec<f32>>` 改为 `EmbeddingRequest -> EmbeddingResponse`），但原签名无人使用，安全变更
- 新增 1 个路由，不影响现有端点

## 测试结果
- `cargo test --workspace`：356 tests passed, 0 failed
- `cargo check --workspace`：0 warnings
- 新增 8 个单元测试（4 core + 4 proto）
- Checklist 全部通过（11 项新增 + 9 项回归）
