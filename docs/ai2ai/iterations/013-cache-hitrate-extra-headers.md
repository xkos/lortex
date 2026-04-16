# 迭代 013: 缓存命中率 + 模型自定义 Header UI

> 分支：main（直接提交）
> 状态：✅ 完成

## 目标
Usage Dashboard 新增缓存命中率指标；Models 编辑对话框补全 extra_headers 编辑 UI。

## 完成内容

### 修改文件
| 文件 | 改动 |
|------|------|
| `crates/server/src/store/traits.rs` | TrendPoint / GroupedUsage 新增 cache_write_tokens / cache_read_tokens 字段 |
| `crates/server/src/store/sqlite.rs` | usage_trend / usage_by_model / usage_by_key 聚合逻辑补全缓存维度 |
| `admin-web/src/views/Usage.vue` | Summary cards 重组为 2 行 + 缓存命中率卡片 + 趋势图 Cache Hit Rate 折线 + TS interface 更新 |
| `admin-web/src/views/Models.vue` | 编辑对话框新增 Custom Headers section — 动态 key-value 编辑器 |
| `admin-web/src/locales/en.ts` | cacheHitRate / extraHeaders / headerKey / headerValue / addHeader |
| `admin-web/src/locales/zh.ts` | 缓存命中率 / 自定义 Header / 键 / 值 / 添加 Header |

### 架构

**Part A — 缓存命中率**
```
UsageSummary (已有 cache 字段)
  ↓ 前端计算
cacheHitRate = cache_read / (cache_read + cache_write) × 100

TrendPoint / GroupedUsage (新增 cache 字段)
  ↓ SQLite 聚合
  ↓ serde 自动序列化
  ↓ 前端趋势图新增 Cache Hit Rate 折线（独立 Y 轴 0-100%）
```

**Part B — 模型自定义 Header UI**
```
Models.vue 编辑对话框
  ├── headerList: Array<{key, value}> (编辑态)
  ├── showEdit → Object.entries(extra_headers) → headerList
  ├── handleSave → headerList → Record<string, string> → API payload
  └── Dynamic rows: input(key) + input(value) + delete button + add button
```

## 关键设计决策
- **缓存命中率前端计算**：后端已有 cache_read / cache_write token 数据，命中率是派生指标，前端计算即可，无需新增后端端点
- **Summary cards 两行布局**：原 6 卡一行 span=4 已满，拆为两行（核心指标 span=6 × 4 + 缓存指标 span=8 × 3）语义分组更清晰
- **趋势图用单条 Hit Rate 线而非两条原始 token 线**：命中率趋势比绝对值更有意义，避免与 requests/credits 尺度冲突
- **Header 编辑态用数组**：Vue 响应式数组比动态 key object 更方便操作（添加/删除/v-for），save 时转回对象

## 未完成/遗留
无

## 回归影响
- TrendPoint / GroupedUsage 新增字段为 additive，不影响已有消费者（前端 JS 忽略多余字段）
- Models.vue 新增 section 不影响已有字段保存逻辑
- 无后端 handler 变更

## 测试结果

### 本迭代新增
- [x] Usage Dashboard — 缓存命中率卡片
- [x] Usage Dashboard — 趋势图缓存折线
- [x] Usage Dashboard — TrendPoint 缓存字段
- [x] Usage Dashboard — GroupedUsage 缓存字段
- [x] Models — Custom Headers 编辑器
- [x] Models — Headers 保存不丢失
- [x] Models — Headers 清空
- [x] i18n 中英文
- [x] `cargo test --workspace` → 356 tests passed, 0 failed

### 回归测试
- [x] server / proxy API / admin API tests passed
- [x] /v1/chat/completions / /v1/messages / /v1/embeddings 正常
- [x] RPM/TPM 限流 / 熔断 / 模型级限流 正常
- [x] Usage Dashboard 原有图表正常
