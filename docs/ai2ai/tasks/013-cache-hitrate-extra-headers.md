# 任务 013: 缓存命中率 + 模型自定义 Header UI

> 状态：✅ 已关闭
> 分支：main（直接提交）
> 配对迭代：[iterations/013-cache-hitrate-extra-headers.md](../iterations/013-cache-hitrate-extra-headers.md)

## 迭代目标
Usage Dashboard 新增缓存命中率指标 + Models 编辑对话框新增 extra_headers 动态编辑 UI。

## 设计摘要

### 核心决策
- 缓存命中率前端计算：`hit_rate = cache_read / (cache_read + cache_write) * 100`，zero-safe
- TrendPoint / GroupedUsage 后端补全 cache 字段，serde 自动带出，无需新增 handler
- extra_headers 编辑态用 `Array<{key, value}>` 方便 UI 操作，save 时转回 `Record<string, string>`

### 不做
- 缓存命中率的"按请求计数"维度（仅按 token 量计算）
- extra_headers 的 key/value 合法性校验（由上游校验）
- 新增聚合端点

## 验收标准
- Usage Dashboard 新增缓存命中率卡片
- 趋势图新增 Cache Hit Rate 折线
- TrendPoint / GroupedUsage API 返回 cache 字段
- Models 编辑对话框可添加/删除 custom headers
- headers 保存后重新编辑不丢失
- `cargo test --workspace` 全量通过
- `npm run build` 编译通过

## 任务分解
- [x] T1: TrendPoint / GroupedUsage 新增 cache_write_tokens / cache_read_tokens 字段
  - 验证：cargo check 通过
- [x] T2: SQLite usage_trend / usage_by_model / usage_by_key 聚合逻辑补全
  - 验证：cargo test 通过
- [x] T3: Usage.vue 缓存命中率卡片 + 趋势图缓存折线 + TS interface 更新
  - 验证：npm run build 通过
- [x] T4: Models.vue extra_headers 动态 key-value 编辑器
  - 验证：npm run build 通过
- [x] T5: i18n — cacheHitRate / extraHeaders 等翻译 key
  - 验证：npm run build 通过
- [x] T6: 全量验证 — cargo test + npm build
  - 验证：356 tests passed, 0 failed; build 成功
