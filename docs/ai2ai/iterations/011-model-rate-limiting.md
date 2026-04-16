# 迭代 011: 模型级限流 + 溢出降级 — per-model RPM/TPM + 自动降级

> 分支：main（直接提交）
> 状态：✅ 完成

## 目标
后端模型有自身 RPM/TPM 上限，本地预判主动将溢出请求降级到同一 ApiKey model_group 中的其他同类型模型，避免不必要的上游 429。

## 完成内容

### 修改文件
| 文件 | 改动 |
|------|------|
| `models/model.rs` | 新增 `rpm_limit` / `tpm_limit` 字段（`#[serde(default)]`） |
| `rate_limiter.rs` | 新增 4 个 per-model 方法 + `model_key()` 辅助函数 + 8 个单元测试 |
| `handlers/shared.rs` | `complete_with_fallback` 加模型限流检查 + `resolve_models_with_fallback` 候选扩展 |
| `handlers/chat.rs` | streaming 路径加模型 RPM/TPM 检查 + record_model_request |
| `handlers/messages.rs` | streaming 路径加模型 RPM/TPM 检查 + record_model_request |
| `layer/usage_layer.rs` | `on_close` 记录 model 级 token（record_model_tokens） |
| `handlers/admin/models.rs` | CreateModelRequest / UpdateModelRequest 增加 rpm_limit / tpm_limit |
| `middleware/proxy_auth.rs` | 测试 — test_model() 补充字段 |
| `store/sqlite.rs` | 测试 — test_model() 补充字段 |
| `tests/proxy_api.rs` | 测试 — Model 构造器补充字段 |
| `admin-web/views/Models.vue` | 编辑对话框新增 Rate Limits 区块（RPM/TPM 输入框） |
| `admin-web/locales/en.ts` | 新增 rateLimits / rpmLimit / tpmLimit / unlimitedHint |
| `admin-web/locales/zh.ts` | 新增对应中文翻译 |

### 架构

```
请求进入
  ↓
resolve_models_with_fallback
  ├── primary model
  ├── fallback_models（ApiKey 配置）
  └── model_group 同类型候选（当 primary 有限流时自动追加）
  ↓
complete_with_fallback / streaming 循环
  ├── circuit_breaker 检查 ← 已有
  ├── model RPM 检查      ← 新增：check_model_rpm
  ├── model TPM 检查      ← 新增：check_model_tpm
  ├── 构建 provider
  ├── 发送请求
  └── 成功 → record_model_request ← 新增
  ↓
UsageLayer on_close
  └── record_model_tokens  ← 新增
```

三层防护互补：
1. **模型限流**（路由选择阶段，主动预判）
2. **熔断器**（provider 级别，连续失败触发）
3. **错误降级**（请求执行阶段，上游失败后切换）

## 关键设计决策
- **check/record 分离**：`check_model_rpm` 只读不记录，`record_model_request` 单独记录。避免检查时产生副作用，只有真正选中模型后才计数
- **"model:" key 前缀**：在同一个 DashMap 中与 per-ApiKey 计数器隔离，无需新增数据结构
- **model_group 自动扩展**：仅当主模型配置了 rpm_limit/tpm_limit 时才追加同类型候选，零配置模型不受影响
- **全局 per-model 维度**：限流计数跨所有 ApiKey 共享，反映真实的上游模型容量

## 未完成/遗留
无

## 回归影响
- Model struct 新增 2 个字段，`#[serde(default)]` 保证已有 JSON 数据向后兼容
- RateLimiter 新增方法，不影响已有 per-ApiKey 逻辑
- 路由循环中新增条件判断，rpm_limit/tpm_limit 为 0 时跳过（零开销）

## 测试结果
- `cargo test --workspace`：348 tests passed, 0 failed
- `npm run build`：编译通过
- 新增 8 个 rate_limiter 单元测试：
  - model_rpm_unlimited / model_rpm_within_limit / model_rpm_exceeds_limit / model_rpm_isolation
  - model_tpm_within_limit / model_tpm_exceeds_limit / model_tpm_zero_not_recorded / model_rpm_window_expires
- Checklist 全部通过（8 项新增 + 8 项回归）
