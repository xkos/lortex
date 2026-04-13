# 迭代 009: Rate Limiting — RPM/TPM per ApiKey

> 分支：iter/009-rate-limiting
> 状态：✅ 完成

## 目标
为每个 ApiKey 提供 RPM（requests per minute）和 TPM（tokens per minute）滑动窗口限流，超限返回 429。

## 完成内容

### 新增文件
| 文件 | 说明 |
|------|------|
| `crates/server/src/rate_limiter.rs` | RateLimiter 核心实现：DashMap + VecDeque 滑动窗口 |

### 修改文件
| 文件 | 改动 |
|------|------|
| `crates/server/Cargo.toml` | 新增 `dashmap = "6"` 依赖 |
| `crates/server/src/lib.rs` | 新增 `pub mod rate_limiter;` |
| `crates/server/src/models/api_key.rs` | 新增 `rpm_limit: u32` + `tpm_limit: u32` 字段 |
| `crates/server/src/handlers/admin/keys.rs` | Create/Update/Response DTO 新增 RPM/TPM 字段 |
| `crates/server/src/middleware/proxy_auth.rs` | RPM/TPM 检查 + `rate_limit_response()` 辅助函数 |
| `crates/server/src/state.rs` | AppState 新增 `Arc<RateLimiter>` + `with_rate_limiter()` |
| `crates/server/src/layer/usage_layer.rs` | on_close 中调用 `record_tokens()` 写入 TPM 窗口 |
| `crates/server/src/bin/proxy.rs` | 共享 RateLimiter 实例，分发给 UsageLayer 和 AppState |
| `crates/server/admin-web/src/views/ApiKeys.vue` | 表格新增 RPM/TPM 列；Create/Edit 表单新增字段 |

### 架构
```
请求 → proxy_auth middleware
        ├── api_key 认证 ✓
        ├── credit 检查 ✓
        ├── RPM 检查 ← RateLimiter.check_rpm()  [NEW]
        ├── TPM 检查 ← RateLimiter.check_tpm()  [NEW]
        └── 注入 ApiKey → handler → LLM 调用
                                        ↓
                                    UsageLayer.on_close()
                                    ├── record_tokens() → RateLimiter  [NEW]
                                    ├── add_credits_used → DB
                                    └── insert_usage → DB
```

## 关键设计决策
- **内存滑动窗口**：DashMap + VecDeque，无需 Redis，适合单实例部署
- **RPM 原子性**：check_rpm 同时检查并记录（原子操作，不会超限）
- **TPM 近似性**：请求进入时检查历史窗口，但当前请求的 token 数在完成前未知，属于 best-effort
- **共享实例**：RateLimiter 由 proxy.rs 创建，同时传给 AppState（check）和 UsageLayer（record）
- **0 = 不限制**：与 credit_limit 语义一致，向后兼容旧数据（`#[serde(default)]`）
- **429 响应头**：OpenAI 兼容格式（x-ratelimit-limit-requests + retry-after）

## 未完成/遗留
无

## 回归影响
- ApiKey JSON 新增 rpm_limit/tpm_limit 字段，旧数据通过 serde(default) 兼容
- proxy_auth 中间件新增两个检查步骤，limit=0 时跳过（零开销）

## 测试结果
- `cargo test --workspace`：全量通过（330+ tests, 0 failed）
- `cargo check -p lortex-server`：0 warnings
- 新增 10 个 rate_limiter 单元测试
