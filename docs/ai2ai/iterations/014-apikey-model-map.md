# 迭代 014: ApiKey 模型映射（占位符 → 实际模型）

> 分支：main（直接提交）
> 状态：✅ 完成

## 目标
ApiKey 新增 model_map 字段，支持客户端模型名到实际模型 ID 的 per-key 映射，兼容 Claude Code 等工具的模型环境变量。

## 完成内容

### 修改文件
| 文件 | 改动 |
|------|------|
| `crates/server/src/models/api_key.rs` | 新增 `model_map: HashMap<String, String>` + `#[serde(default)]` |
| `crates/server/src/handlers/shared.rs` | `resolve_model` 加 model_map 映射（2 行） |
| `crates/server/src/handlers/admin/keys.rs` | Create/Update/Response 透传 model_map |
| `crates/server/src/store/sqlite.rs` | 测试 helper 补全 model_map 字段 |
| `crates/server/src/middleware/proxy_auth.rs` | 测试 ApiKey 构造补全 model_map 字段 |
| `admin-web/src/views/ApiKeys.vue` | Model Mapping section — 动态行编辑器 + Quick Add: Claude Code |
| `admin-web/src/locales/en.ts` | modelMap / placeholder / targetModel / addMapping / quickAddClaude |
| `admin-web/src/locales/zh.ts` | 模型映射 / 占位符 / 目标模型 / 添加映射 / 快捷添加: Claude Code |

### 架构
```
客户端请求: model="ANTHROPIC_DEFAULT_SONNET_MODEL"
  ↓ proxy_auth middleware（鉴权，取出 ApiKey）
  ↓
resolve_model()
  ├── 1. PROXY_MANAGED? → default_model
  ├── 2. model_map 命中? → mapped model ID     ← 新增
  └── 3. 正常 find_model（ID / 别名）
  ↓
model_group 权限检查（不变）
  ↓
继续正常 proxy 流程
```

### Quick Add 预设占位符
- `ANTHROPIC_MODEL`
- `ANTHROPIC_DEFAULT_SONNET_MODEL`
- `ANTHROPIC_DEFAULT_OPUS_MODEL`
- `ANTHROPIC_DEFAULT_HAIKU_MODEL`
- `ANTHROPIC_REASONING_MODEL`

## 关键设计决策
- **model_map 在 JSON blob 中**：SQLite entities 表用 JSON data 列，新增字段 `#[serde(default)]` 零迁移
- **解析优先级 PROXY_MANAGED > model_map > 正常解析**：PROXY_MANAGED 是已有约定，保持优先
- **model_group 权限检查不跳过**：model_map 目标必须在 key 的 model_group 中，保持权限模型一致
- **占位符使用环境变量名**：Claude Code 通过环境变量注入模型名，直接用变量名作为占位符最直接

## 未完成/遗留
无

## 回归影响
- ApiKey struct 新增字段，`#[serde(default)]` 兼容旧数据
- resolve_model 逻辑变更为 additive（新增 else-if 分支），不影响现有路径
- 无现有功能行为变化

## 测试结果

### 本迭代新增
- [x] ApiKeys — Model Mapping section 显示
- [x] ApiKeys — 添加/删除映射
- [x] ApiKeys — Quick Add Claude Code 预设
- [x] ApiKeys — 映射保存不丢失
- [x] ApiKeys — 清空映射
- [x] 解析逻辑 — model_map 命中
- [x] 解析逻辑 — model_map 未命中走正常路径
- [x] PROXY_MANAGED 优先
- [x] i18n 中英文
- [x] `cargo test --workspace` → 356 tests passed, 0 failed

### 回归测试
- [x] server / proxy API / admin API tests passed
- [x] /v1/chat/completions / /v1/messages / /v1/embeddings 正常
- [x] RPM/TPM 限流 / 熔断 / 模型级限流 正常
- [x] Usage Dashboard 缓存命中率 + 原有图表正常
- [x] Models Custom Headers 正常
