# 任务 006b: Fallback 路由 + 健康检测 + Prompt Cache 透传

> 状态：✅ 已关闭
> 分支：iter/006b-fallback-health-cache
> 配对迭代：[iterations/006b-fallback-health-cache.md](../iterations/006b-fallback-health-cache.md)

## 迭代目标
主模型失败时自动 fallback、provider 级熔断保护、prompt cache 字段透传。

## 验收标准（人审核/补充）
- Provider 健康状态模型 + ProxyStore trait 扩展 + SQLite KV 实现
- 熔断器逻辑：连续失败 → Open → 冷却后 HalfOpen → 探测成功 → Closed
- 主模型请求失败时，按 ApiKey.fallback_models 顺序尝试（跳过熔断中的 provider）
- Anthropic ContentBlock 的 cache_control 字段在协议转换中保留
- 客户端 HTTP header 与 model extra_headers 合并（客户端优先）
- `cargo test --workspace` 全量通过

## 任务分解
- [x] T1: ProviderHealthStatus 模型 + ProxyStore trait 扩展 + SQLite 实现
  - 验证：get/upsert health_status 通过单元测试 ✅
- [x] T2: 熔断器服务（CircuitBreaker）
  - 验证：record_success/record_failure 状态转换正确；is_available 判断正确 ✅
- [x] T3: Fallback 路由集成到 handler
  - 验证：主模型失败 → 自动尝试 fallback_models；熔断的 provider 被跳过 ✅
- [x] T4: Prompt cache 透传
  - 验证：Anthropic ContentBlock 带 cache_control 时原样转发；客户端 header 与 extra_headers 合并 ✅
- [x] T5: 测试验证
  - 验证：cargo test --workspace 309 tests passed, 0 failed ✅
