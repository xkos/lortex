# 迭代 006b: Fallback 路由 + 健康检测 + Prompt Cache 透传

> 分支：iter/006b-fallback-health-cache
> 日期：2026-04-11

## 目标
主模型失败时自动 fallback、provider 级熔断保护、prompt cache 字段透传。

## 完成内容

### T1: ProviderHealthStatus 模型 + ProxyStore trait + SQLite 实现
- 新增 `models/health.rs`：`CircuitState` 枚举 + `ProviderHealthStatus` 结构
- `ProxyStore` trait 扩展：`get_health_status` / `upsert_health_status`
- SQLite 实现：`kind = 'health_status'` 存入 entities 表
- 3 个单元测试

### T2: CircuitBreaker 熔断器服务
- 新增 `circuit_breaker.rs`：Closed → Open → HalfOpen 状态机
- 配置：`failure_threshold: 3`, `cooldown_secs: 30`
- `is_available()` / `record_success()` / `record_failure()`
- `AppState` 新增 `circuit_breaker` 字段 + `AppState::new()` 构造器
- 8 个单元测试

### T3: Fallback 路由集成到 handler
- `shared.rs` 新增 `resolve_models_with_fallback()` + `complete_with_fallback()`
- Non-streaming：依次尝试主模型 + fallback，记录熔断状态
- Streaming：pre-stream fallback（选第一个可用模型再开流）
- `is_retriable()` 判断是否可 fallback

### T4: Prompt cache 透传
- `ContentBlock` 4 个变体新增 `cache_control: Option<Value>`
- `MessagesRequest.system` 改为 `Option<Value>` 支持 array 形式
- `merge_headers()`：`anthropic-beta` 逗号去重合并，其余 client 覆盖 model
- `extract_passthrough_headers()` / `build_provider_with_headers()`
- chat.rs / messages.rs 入口提取 HeaderMap 并透传

### T5: 测试验证
- `cargo test --workspace`：309 tests passed, 0 failed
- `cargo clippy --workspace`：无新增 warning

## 未完成/遗留
无

## 回归影响
- `AppState` 构造改为 `AppState::new(store)`，已全部更新
- `ContentBlock` 新增字段，所有 pattern match 已更新

## 测试结果（归档自 checklist）

### 本迭代新增
- [ ] 熔断器：连续失败 3 次后 provider 标记为 Open（8 unit tests 覆盖）
- [ ] Fallback：主模型失败 → 自动尝试 fallback_models（集成测试覆盖）
- [ ] cache_control 透传：ContentBlock 带 cache_control 时原样保留
- [ ] Header 合并：客户端 anthropic-beta 与 model extra_headers 逗号去重合并（4 unit tests）
- [ ] 全量：`cargo test --workspace` → 309 tests passed, 0 failed

### 回归测试
- [ ] server: `cargo test -p lortex-server` → 72 unit + 21 integration tests passed
- [ ] proxy API: `cargo test -p lortex-server --test proxy_api` → 15 tests passed
- [ ] admin API: `cargo test -p lortex-server --test admin_api` → 6 tests passed
- [ ] core + router + others: 全部通过
