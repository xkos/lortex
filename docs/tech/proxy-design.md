# Lortex Proxy — 技术设计

> 状态：草稿
> 最后更新：2026-04-10
> 关联 PRD：[proxy.md](../prds/proxy.md)
> 关联框架架构：[architecture.md](./architecture.md)

---

## 一、Crate 结构

```
lortex/
├── crates/
│   ├── server/          ← 新增：proxy 服务 lib crate
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── config.rs        — 启动配置（端口、db 路径、admin key）
│   │       ├── store/
│   │       │   ├── mod.rs       — ProxyStore trait
│   │       │   └── sqlite.rs    — SQLite 实现
│   │       ├── models/
│   │       │   ├── mod.rs
│   │       │   ├── provider.rs  — Provider 结构体
│   │       │   ├── model.rs     — Model 结构体
│   │       │   └── api_key.rs   — ApiKey 结构体
│   │       ├── router/
│   │       │   └── mod.rs       — 从 store 动态构建 Router
│   │       ├── handlers/
│   │       │   ├── mod.rs
│   │       │   ├── chat.rs      — /v1/chat/completions
│   │       │   ├── messages.rs  — /v1/messages (Anthropic)
│   │       │   ├── models.rs    — /v1/models
│   │       │   └── admin/
│   │       │       ├── mod.rs
│   │       │       ├── providers.rs
│   │       │       ├── models.rs
│   │       │       └── keys.rs
│   │       ├── middleware/
│   │       │   ├── auth.rs      — API Key 鉴权 + credit 检查
│   │       │   └── credits.rs   — 响应后扣减 credit
│   │       └── proto/
│   │           ├── openai.rs    — OpenAI 请求/响应类型
│   │           └── anthropic.rs — Anthropic 请求/响应类型
│   └── ...（现有 crate）
│
└── src/
    └── bin/
        └── proxy.rs             ← 新增：lortex-proxy binary 入口
```

**依赖关系**：
```
server → lortex-core, lortex-router, lortex-providers
proxy binary → server（组装并启动）
```

server crate 不依赖 executor/swarm/memory，职责单一：HTTP 路由、协议转换、存储读写。

---

## 二、数据模型

### 2.1 Provider

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,           // 用户自定义，唯一
    pub vendor: Vendor,       // 枚举：OpenAI | Anthropic | DeepSeek | Custom
    pub display_name: String,
    pub api_key: String,      // 明文存储（SQLite 文件权限控制安全）
    pub base_url: String,     // 厂商 API base URL，支持覆盖
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Vendor {
    OpenAI,
    Anthropic,
    DeepSeek,
    Custom(String),  // 其他兼容 OpenAI 格式的厂商
}
```

### 2.2 Model

```rust
/// 模型类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelType {
    Chat,
    Embedding,
    ImageGeneration,
    Tts,
    Stt,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Model {
    // ID = "{provider_id}/{vendor_model_name}"，由程序构造，不单独存储
    pub provider_id: String,
    pub vendor_model_name: String,
    pub display_name: String,
    pub aliases: Vec<String>,       // 短名称别名，如 ["claude-sonnet"]
    pub model_type: ModelType,

    // 能力声明
    pub supports_streaming: bool,
    pub supports_tools: bool,            // function calling / tool use
    pub supports_structured_output: bool,
    pub supports_vision: bool,           // 图片输入理解
    pub supports_prefill: bool,          // 前缀续写
    pub supports_cache: bool,            // prompt cache
    pub supports_web_search: bool,       // 厂商原生联网搜索
    pub supports_batch: bool,            // 批量推理 API
    pub context_window: u32,

    // 缓存控制
    pub cache_enabled: bool,             // 默认 true，可关闭

    // 文本计费倍率（每 1k tokens 消耗的 credits）
    pub input_multiplier: f64,
    pub output_multiplier: f64,
    pub cache_write_multiplier: Option<f64>,  // None = 不支持缓存计费
    pub cache_read_multiplier: Option<f64>,

    // 多模态计费倍率（None = 不支持该模态）
    pub image_input_multiplier: Option<f64>,       // 每张图片/tile
    pub audio_input_multiplier: Option<f64>,       // 每秒音频
    pub video_input_multiplier: Option<f64>,       // 每秒视频
    pub image_generation_multiplier: Option<f64>,  // 每张生成图片
    pub tts_multiplier: Option<f64>,               // 每 1k 字符

    // 自定义 header（转发请求时自动附加）
    pub extra_headers: HashMap<String, String>,

    pub enabled: bool,
    pub created_at: DateTime<Utc>,
}

impl Model {
    pub fn id(&self) -> String {
        format!("{}/{}", self.provider_id, self.vendor_model_name)
    }

    /// 检查给定名称是否匹配此模型（ID 或任意别名）
    pub fn matches(&self, name: &str) -> bool {
        self.id() == name || self.aliases.iter().any(|a| a == name)
    }
}
```

### 2.3 ApiKey

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: String,             // 内部 UUID
    pub key: String,            // "sk-proxy-" 前缀 + 随机串，客户端使用
    pub name: String,           // 可读名称

    pub model_group: Vec<String>,   // 可用模型 ID 列表（支持 ID 或别名）
    pub default_model: String,      // PROXY_MANAGED 时使用
    pub fallback_models: Vec<String>, // Phase 2：故障切换备选列表

    pub credit_limit: i64,      // 0 = 不限制
    pub credit_used: i64,       // 累计消耗，仅增不减

    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

impl ApiKey {
    /// 计算剩余额度，None 表示不限制
    pub fn remaining_credits(&self) -> Option<i64> {
        if self.credit_limit == 0 {
            None
        } else {
            Some(self.credit_limit - self.credit_used)
        }
    }

    pub fn has_credits(&self) -> bool {
        self.credit_limit == 0 || self.credit_used < self.credit_limit
    }
}
```

---

## 三、存储接口

```rust
#[async_trait]
pub trait ProxyStore: Send + Sync {
    // Provider
    async fn get_provider(&self, id: &str) -> Result<Option<Provider>, StoreError>;
    async fn list_providers(&self) -> Result<Vec<Provider>, StoreError>;
    async fn upsert_provider(&self, p: &Provider) -> Result<(), StoreError>;
    async fn delete_provider(&self, id: &str) -> Result<(), StoreError>;

    // Model
    async fn get_model(&self, provider_id: &str, vendor_model_name: &str)
        -> Result<Option<Model>, StoreError>;
    async fn list_models(&self) -> Result<Vec<Model>, StoreError>;
    async fn list_models_by_provider(&self, provider_id: &str)
        -> Result<Vec<Model>, StoreError>;
    async fn upsert_model(&self, m: &Model) -> Result<(), StoreError>;
    async fn delete_model(&self, provider_id: &str, vendor_model_name: &str)
        -> Result<(), StoreError>;

    // ApiKey
    async fn get_api_key_by_key(&self, key: &str) -> Result<Option<ApiKey>, StoreError>;
    async fn get_api_key_by_id(&self, id: &str) -> Result<Option<ApiKey>, StoreError>;
    async fn list_api_keys(&self) -> Result<Vec<ApiKey>, StoreError>;
    async fn upsert_api_key(&self, k: &ApiKey) -> Result<(), StoreError>;
    async fn delete_api_key(&self, id: &str) -> Result<(), StoreError>;
    async fn add_credits_used(&self, key_id: &str, credits: i64) -> Result<(), StoreError>;
    async fn reset_credits(&self, key_id: &str) -> Result<(), StoreError>;
}
```

SQLite 表结构（无外键约束）：

```sql
CREATE TABLE providers (
    id           TEXT PRIMARY KEY,
    vendor       TEXT NOT NULL,
    display_name TEXT NOT NULL,
    api_key      TEXT NOT NULL,
    base_url     TEXT NOT NULL,
    enabled      INTEGER NOT NULL DEFAULT 1,
    created_at   TEXT NOT NULL
);

CREATE TABLE models (
    provider_id            TEXT NOT NULL,
    vendor_model_name      TEXT NOT NULL,
    display_name           TEXT NOT NULL,
    aliases                TEXT NOT NULL DEFAULT '[]',  -- JSON array
    model_type             TEXT NOT NULL DEFAULT 'chat',

    -- 能力声明
    supports_streaming     INTEGER NOT NULL DEFAULT 1,
    supports_tools         INTEGER NOT NULL DEFAULT 0,
    supports_structured_output INTEGER NOT NULL DEFAULT 0,
    supports_vision        INTEGER NOT NULL DEFAULT 0,
    supports_prefill       INTEGER NOT NULL DEFAULT 0,
    supports_cache         INTEGER NOT NULL DEFAULT 0,
    supports_web_search    INTEGER NOT NULL DEFAULT 0,
    supports_batch         INTEGER NOT NULL DEFAULT 0,
    context_window         INTEGER NOT NULL DEFAULT 0,

    -- 缓存控制
    cache_enabled          INTEGER NOT NULL DEFAULT 1,

    -- 文本计费倍率
    input_multiplier       REAL NOT NULL DEFAULT 1.0,
    output_multiplier      REAL NOT NULL DEFAULT 1.0,
    cache_write_multiplier REAL,  -- NULL = 不支持
    cache_read_multiplier  REAL,

    -- 多模态计费倍率（NULL = 不支持该模态）
    image_input_multiplier       REAL,
    audio_input_multiplier       REAL,
    video_input_multiplier       REAL,
    image_generation_multiplier  REAL,
    tts_multiplier               REAL,

    -- 自定义 header
    extra_headers          TEXT NOT NULL DEFAULT '{}',  -- JSON object

    enabled                INTEGER NOT NULL DEFAULT 1,
    created_at             TEXT NOT NULL,
    PRIMARY KEY (provider_id, vendor_model_name)
);

CREATE TABLE api_keys (
    id               TEXT PRIMARY KEY,
    key              TEXT NOT NULL UNIQUE,
    name             TEXT NOT NULL,
    model_group      TEXT NOT NULL DEFAULT '[]',     -- JSON array
    default_model    TEXT NOT NULL DEFAULT '',
    fallback_models  TEXT NOT NULL DEFAULT '[]',     -- JSON array
    credit_limit     INTEGER NOT NULL DEFAULT 0,
    credit_used      INTEGER NOT NULL DEFAULT 0,
    enabled          INTEGER NOT NULL DEFAULT 1,
    created_at       TEXT NOT NULL,
    last_used_at     TEXT
);

-- 用量记录表（支持缓存和多模态 token 分类统计）
CREATE TABLE usage_records (
    id               TEXT PRIMARY KEY,
    api_key_id       TEXT NOT NULL,
    provider_id      TEXT NOT NULL,
    vendor_model_name TEXT NOT NULL,

    -- token 分类
    input_tokens         INTEGER NOT NULL DEFAULT 0,
    cache_write_tokens   INTEGER NOT NULL DEFAULT 0,
    cache_read_tokens    INTEGER NOT NULL DEFAULT 0,
    output_tokens        INTEGER NOT NULL DEFAULT 0,

    -- 多模态用量
    image_input_units    INTEGER NOT NULL DEFAULT 0,
    audio_input_seconds  REAL NOT NULL DEFAULT 0,
    video_input_seconds  REAL NOT NULL DEFAULT 0,
    image_gen_units      INTEGER NOT NULL DEFAULT 0,
    tts_characters       INTEGER NOT NULL DEFAULT 0,

    -- 计算结果
    credits_consumed     INTEGER NOT NULL DEFAULT 0,

    created_at           TEXT NOT NULL
);

CREATE INDEX idx_usage_api_key ON usage_records(api_key_id);
CREATE INDEX idx_usage_model ON usage_records(provider_id, vendor_model_name);
CREATE INDEX idx_usage_created ON usage_records(created_at);
```

---

## 四、请求处理流程

### 4.1 认证与鉴权中间件

每个 proxy 请求（非 admin）经过：

```
1. 提取 API Key
   - OpenAI 格式：Authorization: Bearer {key}
   - Anthropic 格式：x-api-key: {key}

2. 查 store → ApiKey
   - 不存在 → 401
   - enabled=false → 401

3. 检查 credit
   - has_credits() = false → 429（超额度）

4. 将 ApiKey 注入 request extension，供后续 handler 使用
```

### 4.2 模型解析

```
1. 取请求中的 model 字段
2. 若为 "PROXY_MANAGED" → 使用 api_key.default_model
3. 在 api_key.model_group 中查找匹配项（ID 或别名）
   - 未找到 → 返回 model not found 错误
4. 从 store 加载 Model + Provider
5. 构建 lortex-router 的路由请求
```

### 4.3 协议转换

```
入口协议          内部表示              出口协议
─────────────────────────────────────────────────
OpenAI JSON    →  Vec<Message>  →  按 vendor 选择：
                                   - OpenAI 后端 → OpenAI JSON
                                   - Anthropic 后端 → Anthropic JSON

Anthropic JSON →  Vec<Message>  →  按 vendor 选择：
                                   - Anthropic 后端 → 直接转发
                                   - OpenAI 后端 → 转换为 OpenAI JSON（协议适配）
```

协议转换函数：
- `fn openai_to_messages(req: &OpenAIChatRequest) -> Vec<Message>`
- `fn anthropic_to_messages(req: &AnthropicMessagesRequest) -> Vec<Message>`
- `fn messages_to_openai(msgs: &[Message], ...) -> OpenAIChatRequest`
- `fn messages_to_anthropic(msgs: &[Message], ...) -> AnthropicMessagesRequest`

### 4.4 Credit 扣减

响应成功返回后（含 streaming 结束后）：

```
// 文本 token 计费
credits = (usage.input_tokens / 1000.0) * model.input_multiplier
        + (usage.output_tokens / 1000.0) * model.output_multiplier

// 缓存 token 计费（如有）
if model.cache_write_multiplier.is_some() {
    credits += (usage.cache_write_tokens / 1000.0) * model.cache_write_multiplier
    credits += (usage.cache_read_tokens / 1000.0) * model.cache_read_multiplier
}

// 多模态计费（如有）
credits += usage.image_input_units * model.image_input_multiplier    // 如有
credits += usage.audio_input_seconds * model.audio_input_multiplier  // 如有
// ...其他模态

// 写入用量记录
store.insert_usage_record(UsageRecord { api_key_id, provider_id, model, tokens..., credits })

// 扣减额度
store.add_credits_used(api_key.id, credits.ceil() as i64)
store.update_last_used_at(api_key.id, now())
```

---

## 五、HTTP 路由

### 主端口路由

```
POST /v1/chat/completions   → handlers::chat::handle
GET  /v1/models             → handlers::models::list
POST /v1/embeddings         → 503（暂未实现，接口已定义）
POST /v1/messages           → handlers::messages::handle（Anthropic 格式）
```

### Admin 端口路由（同端口或独立端口，由启动参数决定）

```
GET    /admin/v1/providers          → admin::providers::list
POST   /admin/v1/providers          → admin::providers::create
GET    /admin/v1/providers/:id      → admin::providers::get
PUT    /admin/v1/providers/:id      → admin::providers::update
DELETE /admin/v1/providers/:id      → admin::providers::delete

GET    /admin/v1/models             → admin::models::list
POST   /admin/v1/models             → admin::models::create
GET    /admin/v1/models/:provider_id/:model_name → admin::models::get
PUT    /admin/v1/models/:provider_id/:model_name → admin::models::update
DELETE /admin/v1/models/:provider_id/:model_name → admin::models::delete

GET    /admin/v1/keys               → admin::keys::list
POST   /admin/v1/keys               → admin::keys::create
GET    /admin/v1/keys/:id           → admin::keys::get
PUT    /admin/v1/keys/:id           → admin::keys::update
DELETE /admin/v1/keys/:id           → admin::keys::delete
POST   /admin/v1/keys/:id/reset-credits → admin::keys::reset_credits
```

Admin 路由通过 axum middleware 验证 `admin_key`（`Authorization: Bearer {admin_key}`）。

---

## 六、Streaming 处理

`/v1/chat/completions` 和 `/v1/messages` 都需要支持 SSE streaming：

```
1. 检查请求 stream=true
2. 设置响应头 Content-Type: text/event-stream
3. 创建 axum::response::Sse 响应
4. 调用 provider.complete_stream()
5. 逐个 StreamEvent 转换为 SSE chunk：
   - ContentDelta → data: {"choices":[{"delta":{"content":"..."}}]}
   - Done → data: [DONE]
6. streaming 完成后，根据 usage 信息扣减 credit
   （streaming 时每个 chunk 无 token 计数，Done 事件中有 usage）
```

注意：当前 providers crate 的 streaming 是伪流式（先拿完整 body 再解析），在 proxy 实际接入真实厂商时会触发这个问题。这是框架反向推动优化的第一个案例，记录为技术债，修复作为 003 迭代中的技术子任务。

---

## 七、启动配置

```rust
#[derive(Debug, Clone)]
pub struct ServerConfig {
    /// 主端口（proxy 入口）
    pub port: u16,

    /// admin API 端口，None 表示与主端口合并
    pub admin_port: Option<u16>,

    /// SQLite 数据库文件路径
    pub db_path: String,

    /// admin API 鉴权 key
    pub admin_key: String,

    /// 主机地址
    pub host: String,
}

impl ServerConfig {
    /// 从命令行参数和环境变量读取
    pub fn from_env_and_args() -> Result<Self, ConfigError>;
}
```

---

## 八、Binary 入口

```rust
// src/bin/proxy.rs
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // 1. 初始化日志
    tracing_subscriber::init();

    // 2. 加载配置
    let config = ServerConfig::from_env_and_args()?;

    // 3. 初始化存储
    let store = SqliteStore::new(&config.db_path).await?;
    store.migrate().await?;

    // 4. 启动服务
    lortex_server::start(config, Arc::new(store)).await
}
```

---

## 九、与框架的反向优化点（预期）

| 发现的问题 | 触发场景 | 框架优化方向 |
|-----------|---------|------------|
| Provider streaming 伪流式 | proxy 透传 SSE 时延迟高 | providers crate 改为真实增量 SSE 解析 |
| Router 缺少"按 store 动态构建"能力 | proxy 每次请求动态选 model | Router 支持动态 provider 注册 |
| CostTracker 不支持持久化 | proxy 重启后 credit_used 丢失 | CostTracker 支持 store 后端（proxy 侧用 store 直接写） |
| Provider trait 无连接池 | 高并发下 HTTP 连接复用 | providers 内置 client 复用 |
| Usage 缺少缓存 token 分类 | proxy 需要区分 cache_write/cache_read | core Usage 结构扩展缓存字段 |
| ContentPart 多模态格式不完整 | 跨厂商 Vision 格式转换 | core ContentPart 补充多模态变体 |

---

## 十、003 迭代范围确认

**包含**：
- server crate（lib）：全部上述结构
- proxy binary 入口
- SQLite store 实现（含 migration，表结构包含多模态和缓存字段预留）
- `/v1/chat/completions`（streaming + non-streaming）
- `/v1/messages`（Anthropic 格式入口）
- `/v1/models`（按 API Key 返回模型组）
- Admin CRUD API（Provider / Model / ApiKey）
- API Key 鉴权 + credit 检查 + credit 扣减（含缓存 token 分类计费）
- 模型寻址（PROXY_MANAGED / 完整 ID / 别名）
- 用量记录写入（usage_records 表）
- providers crate 真实 SSE streaming 修复（框架优化子任务）

**不包含（004+）**：
- FallbackRouter 故障切换
- 多模态协议转换（Vision 跨厂商格式适配）
- 多模态端点（`/v1/images/generations`、`/v1/audio/*`）
- `/v1/embeddings` 实现
- 用量统计 API
- 热重载
- 连接池优化
- Proxy 增强能力（联网搜索 tool 注入等）
- 批量推理 API
