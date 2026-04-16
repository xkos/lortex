# 任务 011: 模型级限流 + 溢出降级

> 状态：✅ 已关闭
> 分支：main（直接提交）
> 配对迭代：[iterations/011-model-rate-limiting.md](../iterations/011-model-rate-limiting.md)

## 迭代目标
后端模型有自身 RPM/TPM 上限，当某模型被打满时，本地预判主动将溢出请求降级到同一 ApiKey model_group 中的其他同类型模型，避免不必要的上游 429。

## 设计摘要

### 与现有错误降级的关系
| 维度 | 现有错误降级 | 新增模型限流降级 |
|------|-------------|---------------|
| 触发时机 | 上游返回错误后（被动） | 请求到达时预判（主动） |
| 配置位置 | `ApiKey.fallback_models` | `Model.rpm_limit` / `Model.tpm_limit` |
| 计数器维度 | per-ApiKey | per-Model（跨所有 ApiKey 共享） |
| 复用代码 | `complete_with_fallback` | 复用 `RateLimiter` + 扩展路由选择 |

### 不做
- 动态权重负载均衡（当前是 skip/fallback，不是加权分流）
- per-ApiKey × per-Model 交叉限流（当前 model 限流是全局维度）
- 限流告警/通知

## 验收标准
- Model struct 支持 rpm_limit / tpm_limit 配置
- RateLimiter 支持 per-model 独立计数（与 per-ApiKey 隔离）
- 路由选择阶段主动跳过超限模型
- 主模型超限时自动扩展 model_group 同类型候选
- UsageLayer 记录 model 级 token 消耗
- Admin API + 前端支持配置模型 RPM/TPM
- `cargo test --workspace` 全量通过
- `npm run build` 前端编译通过

## 任务分解
- [x] T1: Model 新增 rpm_limit / tpm_limit 字段
  - `#[serde(default)]` 向后兼容
  - 验证：编译通过，已有 JSON 数据可正常反序列化
- [x] T2: RateLimiter 新增 per-model 方法
  - check_model_rpm / record_model_request / check_model_tpm / record_model_tokens
  - "model:" key 前缀隔离
  - 验证：8 个新单元测试全部通过
- [x] T3: 路由选择 — 主动跳过限流模型
  - complete_with_fallback 中检查 model RPM/TPM
  - streaming 路径（chat.rs / messages.rs）同步处理
  - 验证：编译通过，逻辑正确
- [x] T4: 候选模型扩展 — model_group 同类型降级
  - resolve_models_with_fallback 自动追加同类型候选
  - 验证：编译通过
- [x] T5: UsageLayer 记录 model 级 token
  - on_close 中新增 record_model_tokens 调用
  - 验证：编译通过
- [x] T6: Admin API + 前端
  - CreateModelRequest / UpdateModelRequest 增加字段
  - Models.vue 编辑对话框新增 RPM/TPM 配置
  - i18n 翻译补充
  - 验证：npm run build 成功
- [x] T7: 全量测试
  - cargo test --workspace: 348 tests passed, 0 failed
