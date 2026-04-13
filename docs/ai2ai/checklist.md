# 测试 Checklist

> AI 生成和维护，人审核和勾选。
> 当前迭代：009-rate-limiting

## 本迭代新增

- [x] RPM 限流：创建 rpm_limit=3 的 key → 连发 4 次请求 → 第 4 次返回 429
- [x] TPM 限流：创建 tpm_limit=100 的 key → 发请求消耗 >100 tokens → 下次请求返回 429
- [x] limit=0 无限制：rpm_limit=0 / tpm_limit=0 的 key → 不触发限流
- [x] 429 响应头：超限 429 响应包含 `retry-after` 和 `x-ratelimit-limit-requests` 头
- [x] Admin API 设置：POST /keys 创建时传 rpm_limit/tpm_limit → GET /keys 能看到值
- [x] Admin API 更新：PUT /keys/{id} 修改 rpm_limit/tpm_limit → 生效
- [x] Admin Web 显示：API Keys 列表显示 RPM/TPM 列
- [x] Admin Web 表单：Create/Edit 对话框包含 RPM/TPM 输入框
- [x] 向后兼容：已有 key（无 rpm_limit/tpm_limit 字段）正常工作，默认不限流
- [x] 全量：`cargo test --workspace` → 330+ tests passed, 0 failed

## 回归测试

- [x] server: `cargo test -p lortex-server` → unit + integration tests passed
- [x] proxy API: `cargo test -p lortex-server --test proxy_api` → 15 tests passed
- [x] admin API: `cargo test -p lortex-server --test admin_api` → 6 tests passed
- [x] 熔断器 + Fallback：正常工作（006b 功能不受影响）
- [x] cache_control 透传：正常工作（006b 功能不受影响）
- [x] estimated_chars 记录：发请求后 Usage 表 Est.Chars 列有合理值
- [x] Credit 扣减正常：发请求后 ApiKey credit_used 增加
- [x] Usage 记录完整：Usage 表有 input_tokens/output_tokens/credits/latency_ms/ttft_ms
