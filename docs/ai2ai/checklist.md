# 测试 Checklist

> AI 生成和维护，人审核和勾选。
> 当前迭代：013-cache-hitrate-extra-headers

## 本迭代新增

- [x] Usage Dashboard — 缓存命中率卡片：Summary 区第二行显示"Cache Hit Rate / 缓存命中率"百分比卡片
- [x] Usage Dashboard — 趋势图缓存折线：Daily Trend 图表新增"Cache Hit Rate"虚线折线（右侧第二 Y 轴，0-100%）
- [x] Usage Dashboard — TrendPoint 缓存字段：API 返回的 trend 数据包含 cache_write_tokens / cache_read_tokens
- [x] Usage Dashboard — GroupedUsage 缓存字段：by-model / by-key API 返回数据包含 cache_write_tokens / cache_read_tokens
- [x] Models — Custom Headers 编辑器：模型编辑对话框 Rate Limits 下方显示"Custom Headers"section，可添加/删除 key-value 行
- [x] Models — Headers 保存：添加 headers 后保存，重新编辑该模型 → headers 值不丢失
- [x] Models — Headers 清空：删除所有 header 行后保存 → extra_headers 为 null/空
- [x] i18n：中英文切换后 缓存命中率 / Custom Headers 等新增文本正确显示
- [x] 全量：`cargo test --workspace` → 356 tests passed, 0 failed

## 回归测试

- [x] server: `cargo test -p lortex-server` → unit + integration tests passed
- [x] proxy API: `cargo test -p lortex-server --test proxy_api` → 15 tests passed
- [x] admin API: `cargo test -p lortex-server --test admin_api` → 6 tests passed
- [x] /v1/chat/completions：正常工作不受影响
- [x] /v1/messages：正常工作不受影响
- [x] /v1/embeddings：正常工作不受影响
- [x] RPM/TPM per-ApiKey 限流：正常工作
- [x] 熔断器 + Fallback：正常工作
- [x] 模型级限流 + 溢出降级：正常工作
- [x] Usage Dashboard 原有图表：趋势、模型分布、Key 排行正常显示
