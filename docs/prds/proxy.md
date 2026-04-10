# Lortex Proxy — 产品需求文档

> 状态：已确认
> 最后更新：2026-04-10
> 关联框架需求：[overview.md](./overview.md) 域十一

---

## 一、产品定位

Lortex Proxy 是一个本地/自托管的 LLM 中转代理服务，以独立二进制形式发布（`lortex-proxy`）。

**核心定位**：统一 LLM 接入网关，面向个人开发者和小团队。

**解决的问题**：
- 多个 AI 开发工具（Claude Code、Cursor、Windsurf 等）各自配置 API Key 和 endpoint，更换厂商成本高
- 不同厂商协议不统一（OpenAI 格式 vs Anthropic 格式），工具兼容性参差不齐
- 多厂商账号分散管理，无法统一做额度控制和用量追踪
- 单厂商故障时需要手动切换，无法自动降级

**目标用户**：
- 个人开发者：一人管理多个 AI 工具，通过 proxy 统一出口
- 小团队：团队成员共用一个 proxy 实例，通过 API Key 隔离各成员/项目的用量和权限，无需暴露真实厂商 key

**产品形态**：
- `lortex` crate（lib，框架核心）+ `lortex-proxy` binary（可执行文件）
- 符合 Rust lib + bin 最佳实践

---

## 二、核心概念

### 2.1 Provider（供应商）

持有访问凭证（API Key）和 base URL 的容器，多个模型可以共享同一个 Provider。Provider 是"凭证 + 端点"的封装，不包含模型能力信息。

```
Provider {
    id:           "anthropic-main"          // 用户自定义唯一标识
    vendor:       "anthropic"               // 厂商类型：openai | anthropic | deepseek | ...
    display_name: "Anthropic 主账号"
    api_key:      "sk-ant-..."             // 加密存储
    base_url:     "https://api.anthropic.com"  // 可覆盖，支持中转
    enabled:      true
}
```

### 2.2 Model（模型）

模型是"能力声明 + 倍率"的配置单元，通过 `provider_id` 隐式关联到 Provider（程序逻辑保证，不使用数据库外键约束）。

**模型 ID**：直接使用 `provider_id/vendor_model_name` 的组合形式（如 `anthropic-main/claude-sonnet-4-20250514`），直观且无歧义。换供应商就是删旧建新，不存在迁移场景。

```
Model {
    id:                 "anthropic-main/claude-sonnet-4-20250514"  // provider_id/vendor_model_name
    provider_id:        "anthropic-main"           // 所属 Provider（隐式关联，程序保证）
    vendor_model_name:  "claude-sonnet-4-20250514" // 厂商侧的实际模型名
    display_name:       "Claude Sonnet 4"
    aliases:            ["claude-sonnet", "sonnet"] // 可选的短名称别名

    // 模型类型
    model_type:          "chat"    // chat | embedding | image_generation | tts | stt

    // 能力声明（注册时手动声明，后续可加自动发现）
    supports_streaming:       true
    supports_tools:           true    // function calling / tool use
    supports_structured_output: true  // JSON Schema 结构化输出
    supports_vision:          true    // 图片输入理解
    supports_prefill:         true    // 前缀续写
    supports_cache:           true    // prompt cache
    supports_web_search:      false   // 厂商原生联网搜索
    supports_batch:           false   // 批量推理 API
    context_window:           200000

    // 缓存控制
    cache_enabled:            true    // 默认开启，可关闭

    // 文本计费倍率（每 1k tokens 消耗的 credits）
    input_multiplier:         3.0
    output_multiplier:        15.0
    cache_write_multiplier:   3.75    // null = 不支持缓存计费
    cache_read_multiplier:    0.3     // null = 不支持缓存计费

    // 多模态计费倍率（null = 不支持该模态）
    image_input_multiplier:   null    // 每张图片（或每个 tile）
    audio_input_multiplier:   null    // 每秒音频
    video_input_multiplier:   null    // 每秒视频
    image_generation_multiplier: null // 每张生成图片
    tts_multiplier:           null    // 每 1k 字符

    // 自定义 header（转发请求时自动附加）
    extra_headers:            {}

    enabled: true
}
```

**模型寻址**：客户端发请求时，model 字段支持：
- `PROXY_MANAGED` — 由 proxy 按 API Key 关联的模型组自动选择 default_model
- `provider_id/vendor_model_name`（如 `anthropic-main/claude-sonnet-4-20250514`）— 精确指定
- 注册的别名（如 `claude-sonnet`）— 等价于完整 ID

model 名不在当前 API Key 对应模型组内时，返回标准错误（model not found）。

### 2.3 ApiKey（代理密钥）

客户端接入 proxy 使用的密钥，不是厂商的真实 key。每个 ApiKey 代表一个独立的租户/用途。

```
ApiKey {
    id:            "key_abc123"
    key:           "sk-proxy-xxx..."    // 对客户端可见的密钥
    name:          "cursor-personal"   // 可读名称
    model_group:   [                   // 可用模型列表（使用模型 ID 或别名）
        "anthropic-main/claude-sonnet-4-20250514",
        "openai-main/gpt-4o",
        "openai-main/gpt-4o-mini"
    ]
    default_model: "anthropic-main/claude-sonnet-4-20250514"  // PROXY_MANAGED 时的首选
    fallback_models: ["openai-main/gpt-4o"]  // 故障时的备选顺序（Phase 2 实现）

    // 额度控制（credit 制，按量计费）
    credit_limit:  1_000_000    // 0 = 不限制
    credit_used:   0            // 已消耗，只增不减（手动重置接口可清零）

    enabled: true
    created_at: ...
    last_used_at: ...
}
```

### 2.4 Credit 计费模型

- 基本单位：**credit**，无量纲，用户自定义倍率
- 每次 LLM 调用的 credit 计算：
  ```
  credits = (input_tokens / 1000) × input_multiplier
          + (cache_write_tokens / 1000) × cache_write_multiplier   // 如有
          + (cache_read_tokens / 1000) × cache_read_multiplier     // 如有
          + (output_tokens / 1000) × output_multiplier
          + image_units × image_input_multiplier                    // 如有
          + audio_seconds × audio_input_multiplier                  // 如有
          + ...（其他多模态维度）
  ```
- API Key 有 `credit_limit`，超出后拒绝请求（返回 429）
- 不同模型/不同渠道可以配置不同倍率，体现质量差异
- 缓存 token 的消耗独立追踪，可在用量统计中观测缓存率和缓存节省的 credit

### 2.5 Prompt Cache 控制

- **Model 级别**：`cache_enabled` 字段，默认 true
- **ApiKey 级别**：可覆盖 Model 设置，强制关闭缓存（如测试场景）
- proxy 实现：当 cache 关闭时，转发请求前剥掉 `cache_control` 标记（Anthropic）或不传缓存参数（OpenAI）
- 计费：cache_write 和 cache_read 通过独立 multiplier 计费，厂商响应 usage 中的分类 token 数直接映射
- 统计：用量记录保留 input_tokens / cache_write_tokens / cache_read_tokens / output_tokens 四个分类

### 2.6 Header 透传

- **模型静态 header**：Model 注册时配置 `extra_headers`，每次调用该模型时自动附加到后端请求
- **客户端透传 header**：客户端请求中的部分 header 可透传给后端，通过白名单控制（启动配置或 Model 配置中的 `forwarded_headers`）
- 合并规则：Model 静态 header 优先，防止客户端覆盖模型配置
- 安全：`Authorization`、`x-api-key` 等认证 header 不允许透传

### 2.7 Proxy 增强能力（005+）

对于模型不原生支持的功能，proxy 可以通过内置 tool 注入的方式增强：

| 增强能力 | 实现方式 | 优先级 |
|---------|---------|--------|
| 联网搜索 | 注入 search tool → 拦截 tool_call → 调用搜索 API → 回传结果 | 005 |
| 代码执行 | 注入 code_interpreter tool | 005+ |
| 文件解析 | 注入 file_reader tool（PDF/Word → 文本） | 005+ |
| Vision 降级 | 模型不支持 vision 时，调用 OCR 或其他 vision 模型提取描述 | 005+ |

统一模式：proxy 内置 tool → 注入请求 tools 列表 → 拦截 tool_call → 执行 → 回传 tool_result。

---

## 三、API 接入协议

Proxy 同时支持两种请求格式，内部统一处理后转发给后端厂商：

### 3.1 OpenAI 兼容入口（主要）

| 端点 | 方法 | 说明 | 优先级 |
|------|------|------|--------|
| `/v1/chat/completions` | POST | 对话补全（含 Vision），支持 streaming | P0 |
| `/v1/models` | GET | 按 API Key 返回可用模型列表 | P0 |
| `/v1/embeddings` | POST | 向量嵌入 | P1 |
| `/v1/images/generations` | POST | 图片生成（DALL-E 等） | P2 |
| `/v1/audio/speech` | POST | 文字转语音（TTS） | P2 |
| `/v1/audio/transcriptions` | POST | 语音转文字（STT） | P2 |
| `/v1/audio/translations` | POST | 音频翻译 | P2 |

### 3.2 Anthropic 兼容入口

| 端点 | 方法 | 说明 | 优先级 |
|------|------|------|--------|
| `/v1/messages` | POST | Anthropic Messages API（含 Vision），支持 streaming | P0 |
| `/v1/models` | GET | 与 OpenAI 端点共享逻辑，按 key 返回模型 | P0 |

**协议转换**：Anthropic 格式 → 内部统一格式 → 根据后端厂商选择合适的转换路径（Anthropic 后端直接转发；OpenAI 后端转换为 OpenAI 格式）。

认证方式：
- OpenAI 格式：`Authorization: Bearer sk-proxy-xxx`
- Anthropic 格式：`x-api-key: sk-proxy-xxx`

### 3.3 管理 API

**Base path**: `/admin/v1`（建议通过独立端口或 IP 访问控制保护）

| 端点 | 方法 | 说明 |
|------|------|------|
| `/admin/v1/providers` | GET / POST | 列举/创建 Provider |
| `/admin/v1/providers/{id}` | GET / PUT / DELETE | 查看/更新/删除 Provider |
| `/admin/v1/models` | GET / POST | 列举/创建 Model |
| `/admin/v1/models/{id}` | GET / PUT / DELETE | 查看/更新/删除 Model |
| `/admin/v1/keys` | GET / POST | 列举/创建 ApiKey |
| `/admin/v1/keys/{id}` | GET / PUT / DELETE | 查看/更新/删除 ApiKey |
| `/admin/v1/keys/{id}/reset-credits` | POST | 重置额度 |
| `/admin/v1/usage` | GET | 用量统计（按 key / 按模型 / 按时间段） |

管理 API 本身通过独立的 `admin_key`（启动参数或环境变量）保护，无需完整的用户系统。

---

## 四、存储设计

供应商、模型、API Key 是动态数据，使用可插拔存储接口。

```rust
// 存储 trait — 可替换实现
trait ProxyStore: Send + Sync {
    async fn get_provider(&self, id: &str) -> Result<Option<Provider>>;
    async fn list_providers(&self) -> Result<Vec<Provider>>;
    async fn save_provider(&self, p: &Provider) -> Result<()>;
    async fn delete_provider(&self, id: &str) -> Result<()>;

    async fn get_model(&self, id: &str) -> Result<Option<Model>>;
    async fn list_models(&self) -> Result<Vec<Model>>;
    async fn save_model(&self, m: &Model) -> Result<()>;

    async fn get_api_key(&self, key: &str) -> Result<Option<ApiKey>>;
    async fn list_api_keys(&self) -> Result<Vec<ApiKey>>;
    async fn save_api_key(&self, k: &ApiKey) -> Result<()>;
    async fn increment_credits(&self, key_id: &str, credits: f64) -> Result<()>;
}
```

默认实现：**SQLite**（通过 `sqlx`），单文件，无需额外基础设施，适合个人和小团队部署。

存储接口设计让后续可以接入 PostgreSQL（团队规模扩大时）或 Redis（高频读缓存）。

---

## 五、服务架构

```
客户端 (Claude Code / Cursor / Windsurf / 游戏 / AI 工具)
    │
    │ OpenAI 格式 /v1/*
    │ 或 Anthropic 格式 /v1/messages
    ▼
┌─────────────────────────────────────────┐
│  lortex-proxy binary (Axum HTTP Server) │
│                                         │
│  ┌─────────────┐   ┌─────────────────┐ │
│  │ 协议解析层   │   │  管理 API 层     │ │
│  │ (OpenAI /   │   │ /admin/v1/*     │ │
│  │  Anthropic) │   └─────────────────┘ │
│  └──────┬──────┘           │           │
│         │                  │           │
│  ┌──────▼──────────────────▼────────┐  │
│  │         请求处理核心              │  │
│  │  1. API Key 鉴权                 │  │
│  │  2. 模型解析 (PROXY_MANAGED 等)  │  │
│  │  3. Credit 检查                  │  │
│  │  4. 协议转换 (→ Lortex Message)  │  │
│  └──────────────┬───────────────────┘  │
│                 │                       │
│  ┌──────────────▼───────────────────┐  │
│  │    Router (lortex-router)         │  │
│  │    Provider 分发 + 故障切换       │  │
│  └──────────────┬───────────────────┘  │
│                 │                       │
│  ┌──────────────▼───────────────────┐  │
│  │    CostTracker (credit 扣减)      │  │
│  └──────────────┬───────────────────┘  │
└─────────────────┼───────────────────────┘
                  │
         ┌────────┴────────┐
         ▼                 ▼
  OpenAI Provider   Anthropic Provider
  (lortex-providers)
```

---

## 六、部署形态

### 启动参数

```bash
# 最简单（合并端口）
lortex-proxy --db ./lortex.db --port 8080 --admin-key "your-admin-key"

# 分离端口（admin 单独端口，可通过防火墙隔离）
lortex-proxy --db ./lortex.db --port 8080 --admin-port 8081 --admin-key "your-admin-key"

# 环境变量方式
LORTEX_DB=./lortex.db LORTEX_PORT=8080 LORTEX_ADMIN_KEY=secret lortex-proxy
```

admin 端口默认与主端口合并（路径前缀 `/admin/v1`），通过 `--admin-port` 可分离为独立端口。小型/个人部署用合并端口，团队部署建议分离以便独立做网络访问控制。

### 典型配置流程

```
1. 启动 proxy
2. 通过管理 API 添加 Provider（填入真实厂商 API Key）
3. 通过管理 API 注册 Model（声明能力 + 倍率）
4. 通过管理 API 创建 ApiKey（绑定模型组 + 额度）
5. 将各工具的 endpoint 指向 proxy，API Key 填入 step 4 生成的 key
```

---

## 七、迭代计划

### 003 — MVP（本迭代）

**目标**：能将 Claude Code 和 Cursor 的请求接入 proxy，完成基本路由和鉴权。

| 功能 | 说明 |
|------|------|
| axum HTTP 服务 | 监听 OpenAI + Anthropic 两个入口 |
| `/v1/chat/completions` | 支持 streaming（SSE）和非 streaming |
| `/v1/messages` | Anthropic 格式，转换为内部格式后路由 |
| `/v1/models` | 按 API Key 返回模型组 |
| API Key 鉴权 | 验证 key 有效性，检查额度 |
| Credit 扣减 | 每次调用后更新 credit_used |
| 管理 API | Provider / Model / ApiKey 的 CRUD |
| SQLite 存储 | 通过 sqlx 实现 ProxyStore |
| FixedRouter 路由 | 按 key 配置的模型选 provider |
| server crate | 作为 lib crate，实现所有上述功能 |
| lortex-proxy bin | 入口 binary，组装 server + 存储 + 启动 |

**不做**：
- FallbackRouter 故障切换
- 热重载
- Embeddings 端点（定义接口��不实现）
- 用量统计 API

### 004 — 故障切换 + 多模态适配

| 功能 | 说明 |
|------|------|
| FallbackRouter | 主模型失败时自动切换备选 |
| 熔断机制 | 连续失败 N 次标记为不可用 |
| `/v1/embeddings` | Embeddings 端点实现 |
| 多模态协议转换 | 跨厂商的 Vision/图片格式适配 |
| 用量统计 API | 按 key / 模型 / 时间段查询，含缓存率统计 |
| 配置热重载 | 管理 API 变更实时生效，无需重启 |

### 005 — 生产就绪 + 增强能力

| 功能 | 说明 |
|------|------|
| 结构化日志 | 每次请求完整 trace（含 token 用量、延迟、cost） |
| 速率限制 | 每个 ApiKey 的 RPM/TPM 限制 |
| 自动模型发现 | 从厂商 `/models` 接口自动同步可用模型 |
| 请求重试 | 可配置的重试策略（退避、最大次数） |
| 多模态端点 | `/v1/images/generations`、`/v1/audio/*` |
| Proxy 增强 | 联网搜索 tool 注入、代码执行等 |
| 批量推理 | `/v1/batches` 异步批量 API |

---

## 八、与框架的关系

这个 proxy 是框架能力的最佳验证场地：

- **Router crate** — 直接用于模型路由，验证 RoutingStrategy 接口的实际可用性
- **Providers crate** — 验证 Provider trait 的协议适配完整性
- **CostTracker** — 在真实流量下验证计费准确性
- **事件系统** — 请求日志直接消费 RunEvent，验证可观测性设计

发现框架层的问题（缺少的 API、不合理的抽象）直接反馈到框架迭代。

---

## 九、设计决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 模型 ID 格式 | `provider_id/vendor_model_name` | 直观无歧义，换供应商删旧建新，无迁移场景 |
| Provider-Model 关联 | 程序隐式关联，无数据库外键 | 简化 SQLite 部署，关联由应用层保证 |
| 额度计费单位 | Credit 倍率制 | 灵活体现不同模型/渠道的质量差异，中转站生态已验证 |
| Credit 重置 | 只做手动重置接口，不做自动重置 | 按量计费更简单，自动重置后续按需加 |
| Admin 端口 | 参数控制，默认合并，可分离 | 兼顾个人用户（简单）和团队用户（安全隔离）两种场景 |
| 模型别名 | 注册时声明 aliases 列表 | 让客户端可填短名称，不绑定完整 ID |
| 存储 | 可插拔 trait + 默认 SQLite | 单文件部署，后续可接入 PostgreSQL |
| 缓存计费 | 独立 cache_write/cache_read multiplier | 精确反映厂商三档计费，支持缓存率统计 |
| 缓存控制 | Model 级别 cache_enabled，默认开启 | 可按模型/场景灵活关闭 |
| 多模态计费 | 每种 modality 独立 multiplier（Option） | None 同时表示不支持该模态，计费和能力声明合二为一 |
| 模型类型 | model_type 枚举区分 Chat/Embedding/ImageGen/TTS/STT | 路由时按端点类型匹配对应 model_type |
| Header 透传 | 白名单机制 + Model 静态 header 优先 | 防止客户端覆盖认证 header，同时支持灵活扩展 |
| 多模态协议转换 | 004 实现，003 只定义数据结构 | 003 建表时预留字段，避免后续 ALTER TABLE |
| Proxy 增强能力 | 内置 tool 注入模式，005+ 实现 | 统一模式：注入 tool → 拦截 tool_call → 执行 → 回传 |

---
