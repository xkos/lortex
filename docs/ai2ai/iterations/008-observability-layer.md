# 迭代 008: tracing 观测架构改造

> 分支：iter/008-observability-layer
> 状态：✅ 完成

## 目标
用 `tracing::Span` + 自定义 `UsageLayer` 集中处理请求观测逻辑，替代 4 条 handler 路径中散布的手动计时、`deduct_credits` 调用和日志输出。

## 完成内容

### 新增文件
| 文件 | 说明 |
|------|------|
| `crates/server/src/layer/mod.rs` | 观测层模块根 |
| `crates/server/src/layer/span_data.rs` | SpanData + SpanTiming + Visit trait 实现 |
| `crates/server/src/layer/helpers.rs` | record_model_fields / record_usage_fields 辅助函数 |
| `crates/server/src/layer/usage_layer.rs` | UsageLayer 核心实现 |

### 修改文件
| 文件 | 改动 |
|------|------|
| `Cargo.toml` (workspace) | tracing-subscriber 加 `"registry"` feature |
| `crates/server/src/lib.rs` | 加 `pub mod layer;` |
| `crates/server/src/bin/proxy.rs` | Registry + fmt + env-filter + UsageLayer 栈；store 初始化前移 |
| `crates/server/src/middleware/proxy_auth.rs` | 提取 `compute_credits` 纯函数；删除 `deduct_credits` |
| `crates/server/src/middleware/mod.rs` | re-export `compute_credits` 替代 `deduct_credits` |
| `crates/server/src/handlers/chat.rs` | 4 处改造为 span.record() |
| `crates/server/src/handlers/messages.rs` | 同上 |

### 架构
```
Handler                              UsageLayer (tracing::Layer)
  |                                        |
  |-- span = info_span!(                   |-- on_new_span: 存 Instant + SpanData
  |     target: "lortex::usage",           |
  |     api_key_id, endpoint, ...          |
  |   )                                    |
  |                                        |
  |-- span.record("model_id", ...)         |-- on_record: 更新 SpanData
  |-- span.record("input_tokens", ...)     |
  |                                        |
  |-- span dropped                         |-- on_close:
  |                                        |     1. latency_ms = Instant.elapsed()
  |                                        |     2. compute_credits()
  |                                        |     3. tokio::spawn {
  |                                        |        add_credits_used + insert_usage
  |                                        |        + tracing::info!(target: "lortex::layer")
  |                                        |     }
```

## 关键设计决策
- **纯 tracing::Layer**：不依赖 tower-http（streaming 场景下 on_response 太早）
- **on_close + tokio::spawn**：on_close 是同步回调，异步写库需要 spawn
- **target 过滤**：`"lortex::usage"` 用于 proxy 请求 span，`"lortex::layer"` 用于 Layer 自身日志避免递归
- **TTFT**：streaming handler 保留 `Instant::now()` 仅用于计算 TTFT，通过 `span.record("ttft_ms", val)` 传递给 Layer
- **Blocking fallback**：blocking 路径不设 ttft_ms，Layer 在 on_close 自动回退 `ttft_ms = latency_ms`

## 未完成/遗留
无

## 回归影响
- Handler 不再直接调用 `deduct_credits`，credit 扣减和 UsageRecord 写库由 Layer 异步执行
- 日志格式从 handler 手动 `tracing::info!` 改为 Layer 统一 `"Proxy request completed"`
- proxy.rs 中 tracing 初始化顺序变更（store → tracing → info log）

## 测试结果
- `cargo test --workspace`：全量通过（320+ tests, 0 failed）
- `cargo check -p lortex-server`：0 warnings
