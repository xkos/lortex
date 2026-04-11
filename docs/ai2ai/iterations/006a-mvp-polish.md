# 迭代 006a: MVP 补齐

> 分支：iter/006a-mvp-polish
> 状态：已完成

## 目标
补齐 MVP 阶段的粗糙边：Model update 端点、extra_headers 注入、resolve_model 去重、cache token 支持。

## 完成内容

### T1: Model update endpoint + Admin Web 编辑 (`6ba9e33`)
- `UpdateModelRequest` 支持全部可选字段，含 `Option<Option<f64>>` nullable multiplier
- `update()` handler：get-modify-upsert 模式
- Admin Web `handleSave()` 改为编辑时 PUT、新建时 POST

### T2: extra_headers 注入 (`9ef7f2a`)
- OpenAI / Anthropic provider 新增 `extra_headers` 字段 + `with_extra_headers()` builder
- 4 处请求构造（complete/complete_stream × 2 providers）全部注入

### T3: resolve_model + build_provider 去重 (`9ef7f2a`)
- 新建 `handlers/shared.rs`：`ProxyError`, `resolve_model()`, `build_provider()`, `map_provider_error()`
- chat.rs / messages.rs 各删 ~90 行重复代码，改用 `shared::*` + 格式转换函数

### T4: core Usage cache token (`acf8483`)
- `Usage` 新增 `cache_creation_input_tokens` / `cache_read_input_tokens`（`#[serde(default)]`）
- OpenAI: 解析 `prompt_tokens_details.cached_tokens`
- Anthropic: 解析 `cache_creation_input_tokens` / `cache_read_input_tokens`（complete + stream）
- 4 处 `deduct_credits` 调用传递真实 cache token 值

### T5: 测试验证
- `cargo test --workspace`: 289 tests passed, 0 failed

## 未完成/遗留
无。

## 回归影响
- `Usage` struct 新增 2 字段，所有构造点已更新
- `#[serde(default)]` 保证向后兼容

## 测试结果

### 本迭代新增
- [x] Model Update API
- [x] Admin Web 编辑
- [x] extra_headers 注入
- [x] resolve_model 去重
- [x] cache token 传递
- [x] 全量 289 tests passed

### 回归测试
- [x] server: 57 unit + 21 integration
- [x] proxy API: 15 tests
- [x] admin API: 6 tests
- [x] core + router + others: 全通过
