# 任务 009: Rate Limiting — RPM/TPM per ApiKey

> 状态：✅ 已关闭
> 分支：iter/009-rate-limiting
> 配对迭代：[iterations/009-rate-limiting.md](../iterations/009-rate-limiting.md)

## 迭代目标
为每个 ApiKey 提供 RPM（requests per minute）和 TPM（tokens per minute）限流，超限返回 429。

## 设计摘要

### 数据模型
- `ApiKey` 新增 `rpm_limit: u32` 和 `tpm_limit: u32`（0 = 不限制，与 credit_limit 语义一致）
- JSON entity store，无需 SQL migration，`#[serde(default)]` 兼容旧数据

### 核心组件
- `RateLimiter`：内存滑动窗口计数器，`DashMap<String, VecDeque<Instant>>` (RPM) + `DashMap<String, VecDeque<(Instant, u32)>>` (TPM)
- RPM：请求进入时检查 + 记录
- TPM：请求进入时检查历史窗口；请求完成后由 UsageLayer 记录 token 数

### 集成点
- RPM 检查：在 `proxy_auth` 中，api_key 验证通过后、注入 extensions 前
- TPM 检查：同上位置，检查过去 60s 的累计 tokens
- TPM 记录：UsageLayer `on_close` 回调中，写库同时写入 RateLimiter
- `AppState` 新增 `Arc<RateLimiter>` 字段

### 响应
- 超限返回 429 + `ErrorResponse::rate_limit("RPM/TPM limit exceeded")`
- 响应头：`x-ratelimit-limit-requests`、`x-ratelimit-remaining-requests`、`x-ratelimit-reset-requests`（OpenAI 兼容格式）

## 验收标准（人审核/补充）
- RPM 限流生效：连续快速发请求超过 rpm_limit 后返回 429
- TPM 限流生效：累计 token 超过 tpm_limit 后返回 429
- limit=0 时无限制（向后兼容）
- Admin API 可设置/查看 rpm_limit 和 tpm_limit
- 已有功能不受影响（credit、熔断、cache 等）

## 任务分解
- [x] T1: ApiKey 模型 + Admin DTO 扩展
  - 验证：ApiKey 结构体有 rpm_limit/tpm_limit 字段；Create/Update/Response DTO 包含这两个字段；`cargo test` 通过
- [x] T2: RateLimiter 核心实现
  - 验证：单元测试覆盖 check_rpm/record_rpm/check_tpm/record_tokens；滑动窗口正确清理过期条目
- [x] T3: proxy_auth 集成 RPM + TPM 检查
  - 验证：rpm_limit>0 时超限返回 429；tpm_limit>0 时超限返回 429；limit=0 时不检查
- [x] T4: UsageLayer 集成 TPM 记录
  - 验证：请求完成后 token 数写入 RateLimiter；后续请求能看到累计值
- [x] T5: 响应头 + Admin Web
  - 验证：429 响应包含 x-ratelimit-* 头；Admin Web 表单/列表显示 RPM/TPM 列
- [x] T6: 全量测试 + 归档
  - 验证：`cargo test --workspace` 全量通过；checklist 生成
