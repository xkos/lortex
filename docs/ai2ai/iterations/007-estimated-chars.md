# 迭代 007: 请求字符数估算（estimated_chars）

> 完成日期：2026-04-13
> 分支：iter/007-estimated-chars
> 任务文件：[tasks/007-estimated-chars.md](../tasks/007-estimated-chars.md)

## 目标
在 UsageRecord 中记录 proxy 本地计算的请求字符数，用于与上游 input_tokens 对比检测中转商异常计费。

## 完成内容

### T1: UsageRecord 模型 + deduct_credits 签名扩展
- `UsageRecord` 新增 `estimated_chars: u64`，`#[serde(default)]` 保持向后兼容
- `deduct_credits()` 新增 `estimated_chars` 参数，传入 UsageRecord 构造

### T3: Handler 集成
- 4 条路径（chat blocking/streaming、messages blocking/streaming）均在请求发送前计算 `serde_json::to_string(&req).len()`
- 将 `estimated_chars` 传入 `deduct_credits`

### T4: Admin Web — Usage.vue
- 新增 Est.Chars 列，位于 Cache R 和 Credits 之间
- 值为 0 时显示 `-`

### T5: 全量测试
- `cargo test --workspace` → 324 tests passed, 0 failed
- `cargo clippy --workspace` → 无新增 warning

## 未完成/遗留
无

## 回归影响
- 改动仅新增字段和参数，不影响已有功能
- KV store（JSON 序列化）自动包含新字段，`#[serde(default)]` 保证旧数据兼容

## 测试结果

### 本迭代新增
- [x] estimated_chars 记录：发一个简单请求，Usage 表中 Est.Chars 列有合理值
- [x] 异常检测对比：Est.Chars 值与 Input tokens 对比正常
- [x] 旧数据兼容：历史 Usage 记录的 Est.Chars 列显示 `-`
- [x] Usage 页面：Est.Chars 列位于 Cache R 和 Credits 之间
- [x] 全量：`cargo test --workspace` → 324 tests passed, 0 failed

### 回归测试
- [x] server: `cargo test -p lortex-server` → unit + integration tests passed
- [x] proxy API: `cargo test -p lortex-server --test proxy_api` → 15 tests passed
- [x] admin API: `cargo test -p lortex-server --test admin_api` → 6 tests passed
- [x] 熔断器 + Fallback：正常工作
- [x] cache_control 透传：正常工作
