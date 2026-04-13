# 任务 008: tracing 观测架构改造

> 状态：✅ 已关闭
> 分支：iter/008-observability-layer
> 配对迭代：[iterations/008-observability-layer.md](../iterations/008-observability-layer.md)

## 迭代目标
用 tracing::Span + 自定义 UsageLayer 集中处理请求观测，消除 handler 中散布的计时、credit 计算、UsageRecord 写库和日志逻辑。

## 验收标准
- Handler 代码中无 `deduct_credits` 调用、无手动 `tracing::info!("...done")` 日志
- Blocking 路径无手动 `Instant::now()`（Layer 自动计时）
- Streaming 路径保留 `Instant::now()` 仅用于 TTFT
- UsageLayer 在 span 关闭时自动完成 credit 计算、配额扣减、UsageRecord 写库和结构化日志
- `cargo test --workspace` 全量通过

## 任务分解
- [x] T1: SpanData + SpanTiming + Visit 实现
  - 验证：编译通过
- [x] T2: compute_credits 纯函数提取
  - 验证：现有 credit 计算测试通过
- [x] T3: UsageLayer 核心实现（on_new_span / on_record / on_close）
  - 验证：编译通过，单元测试通过
- [x] T4: Subscriber 栈改造（proxy.rs + Cargo.toml）
  - 验证：编译通过，全量测试通过
- [x] T5: Handler 改造 — span.record() 替代 deduct_credits
  - 验证：4 条 handler 路径改造完成，无 deduct_credits 调用
- [x] T6: 清理 + 全量测试
  - 验证：`cargo test --workspace` 全量通过，无编译警告
