# 任务 014: ApiKey 模型映射（占位符 → 实际模型）

> 状态：✅ 已关闭
> 分支：main（直接提交）
> 配对迭代：[iterations/014-apikey-model-map.md](../iterations/014-apikey-model-map.md)

## 迭代目标
ApiKey 新增 model_map 字段，支持客户端模型名（如 Claude Code 环境变量）到实际模型 ID 的 per-key 映射。

## 设计摘要

### 核心决策
- `model_map: HashMap<String, String>` 存储在 ApiKey JSON blob 中，`#[serde(default)]` 兼容旧数据
- resolve_model 在 PROXY_MANAGED 之后、find_model 之前查找 model_map
- model_group 权限检查保持不变 — model_map 目标必须在 key 的 model_group 中

### 不做
- 映射级联（target 再次 mapping）
- 通配符匹配
- 全局默认 model_map
- 目标合法性校验

## 验收标准
- ApiKeys 编辑对话框可添加/删除 model mapping
- Quick Add 按钮填入 Claude Code 环境变量占位符
- mapping 保存后重新编辑不丢失
- 请求中使用占位符名时 proxy 解析到配置的目标模型
- `cargo test --workspace` 全量通过

## 任务分解
- [x] T1: ApiKey 新增 model_map 字段
  - 验证：cargo check 通过
- [x] T2: resolve_model 加 model_map 查找
  - 验证：cargo check 通过
- [x] T3: Admin API Create/Update/Response 透传 model_map
  - 验证：cargo check 通过
- [x] T4: ApiKeys.vue Model Mapping 编辑器 + Quick Add
  - 验证：npm run build 通过
- [x] T5: i18n 翻译 key
  - 验证：npm run build 通过
- [x] T6: 全量验证 — cargo test + npm build
  - 验证：356 tests passed, 0 failed; build 成功
