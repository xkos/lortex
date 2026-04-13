# 测试 Checklist

> AI 生成和维护，人审核和勾选。
> 当前迭代：008-observability-layer

## 本迭代新增

- [x] Handler 无 deduct_credits：grep `deduct_credits` → handlers 中无调用
- [x] Handler 无手动日志：grep `"done"` → handlers 中无手动 `tracing::info!("...done")`
- [x] Blocking 无手动计时：chat/messages blocking 路径中无 `Instant::now()`
- [x] Streaming TTFT 记录：发 streaming 请求 → Usage 表 ttft_ms < latency_ms
- [x] Blocking ttft 回退：发 blocking 请求 → Usage 表 ttft_ms = latency_ms
- [x] Credit 扣减正常：发请求后 ApiKey credit_used 增加
- [x] Usage 记录完整：Usage 表有 input_tokens/output_tokens/credits/latency_ms/ttft_ms
- [x] 结构化日志：日志中出现 "Proxy request completed" 包含 model/provider/tokens/credits 字段
- [x] 全量：`cargo test --workspace` → 320+ tests passed, 0 failed

## 回归测试

- [x] server: `cargo test -p lortex-server` → unit + integration tests passed
- [x] proxy API: `cargo test -p lortex-server --test proxy_api` → 15 tests passed
- [x] admin API: `cargo test -p lortex-server --test admin_api` → 6 tests passed
- [x] 熔断器 + Fallback：正常工作（006b 功能不受影响）
- [x] cache_control 透传：正常工作（006b 功能不受影响）
- [x] estimated_chars 记录：发请求后 Usage 表 Est.Chars 列有合理值
