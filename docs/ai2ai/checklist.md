# 测试 Checklist

> AI 生成和维护，人审核和勾选。
> 当前迭代：011-model-rate-limiting

## 本迭代新增

- [x] Model 字段：创建模型时可配置 rpm_limit / tpm_limit，保存后重新加载值不丢失
- [x] RateLimiter 隔离：model 级计数器与 ApiKey 级计数器互不影响
- [x] RPM 超限跳过：配置 rpm_limit=2 的模型，连续发 3 个请求，第 3 个应跳过该模型
- [x] TPM 超限跳过：配置 tpm_limit 的模型，token 用量超限后请求跳过该模型
- [x] 候选扩展：主模型超限后自动降级到 model_group 中同类型的其他模型
- [x] Admin 前端：Models 编辑对话框显示 RPM Limit / TPM Limit 输入框，0 = 不限制
- [x] 向后兼容：已有模型数据（无 rpm_limit/tpm_limit 字段）可正常加载，默认为 0
- [x] 全量：`cargo test --workspace` → 348 tests passed, 0 failed

## 回归测试

- [x] server: `cargo test -p lortex-server` → unit + integration tests passed
- [x] proxy API: `cargo test -p lortex-server --test proxy_api` → 15 tests passed
- [x] admin API: `cargo test -p lortex-server --test admin_api` → 6 tests passed
- [x] RPM/TPM per-ApiKey 限流：正常工作（009 功能不受影响）
- [x] 熔断器 + Fallback：正常工作（006b 功能不受影响）
- [x] Credit 扣减正常：发请求后 ApiKey credit_used 增加
- [x] Usage Dashboard：趋势/模型/ApiKey 图表正常（010 功能不受影响）
- [x] Usage 记录完整：Usage 表有 input_tokens/output_tokens/credits/latency_ms/ttft_ms
