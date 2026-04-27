# Provider UA 路由与透传 Header 白名单

> 最后更新：2026-04-27
> 关联：[proxy-design.md](./proxy-design.md)
> 相关代码：`crates/server/src/handlers/shared.rs`、`crates/server/src/handlers/provider_builder.rs`、`crates/server/src/handlers/passthrough.rs`

## 一、背景

部分第三方 Anthropic/OpenAI 兼容中转 provider（典型如 `cursorlinkai.com`）会**基于 User-Agent 做路由、限流或静默丢弃**，且策略不透明：

- UA 形如 `curl/*`、官方 SDK 的 UA → 进入正常处理路径
- UA 是 `reqwest/*`、`Mozilla/*`、自定义名称 → 路由到黑洞路径，**返回头都不发，连接空跑 15~50 秒后被砍**
- 某些 UA 叠加 `anthropic-beta: prompt-caching-*` 等 header → 进入另一条更慢/更易失败的路径

直接 curl（默认 UA `curl/x.y`）和官方 Claude Code（`anthropic-ai/*` 或 `claude-cli/*`）不会触发这类问题，所以从业务侧只看 provider 是"能手工调通"的，但经 proxy 的任意客户端（如 maestro、自研 agent）会稳定挂起。

## 二、典型症状

Proxy 日志：

```
WARN lortex_server::handlers::messages: Upstream stream error:
  Upstream network error: error sending request for url (https://cursorlinkai.com/v1/messages)
  | caused by: client error (SendRequest)
  | caused by: connection closed before message completed
status=500 latency_ms=15812 passthrough=true
```

特点：
- 延迟稳定在 ~15~30 秒（上游黑洞路径的空跑超时）
- 错误是 hyper 的 `IncompleteMessage`（上游在 chunked/SSE 中途关 TCP）
- curl 直连同一上游工作正常

## 三、诊断矩阵

以同一个 136 KB 的实际请求体（maestro 发的、包含 7 个 tools、1 条 message）对 `cursorlinkai.com` 直接测试：

| 场景 | User-Agent | 额外 header | HTTP | 耗时 | 字节 |
|------|------------|-------------|------|------|------|
| A | `curl/8.7.1`（curl 默认） | — | 200 | 46 s | 81 KB ✅ |
| B | `Claude-Code/1.0 (Anthropic; en-US)` | `anthropic-beta: prompt-caching-2024-07-31` | 000 | 48 s | 0 ❌ |
| C | `reqwest/0.12`（复现 proxy 行为） | — | 000 | **18 s** | 0 ❌ |
| D | `Mozilla/5.0` | — | 000 | **18 s** | 0 ❌ |
| **proxy 实测** | reqwest 默认 | — | 500 | **15~26 s** | 0 ❌ |

关键观察：
1. C、D 的失败时长和 proxy 实测完全吻合 → proxy 失败就是"UA 命中黑洞"
2. 同样是"非 curl UA"，B 因叠加 prompt-caching beta 被路由到另一条路径，表现不一样
3. curl 默认 UA 是唯一稳定成功的组合

## 四、根因

Proxy 在升级前只透传两个 header：

```rust
// crates/server/src/handlers/shared.rs（改造前）
const PASSTHROUGH_HEADERS: &[&str] = &["anthropic-beta", "anthropic-version"];
```

客户端（maestro/Claude Code）发的 `user-agent`、`accept`、`x-stainless-*` 等都被丢弃，reqwest 对上游用的是自己的默认 UA `reqwest/x.y.z`——正好命中 cursorlinkai 的黑洞规则。

## 五、修复策略

### 1. 默认透传 User-Agent（已做）

把 `user-agent` 加入透传白名单，保持"透明代理"语义——客户端发什么 UA，上游就看到什么 UA。

```rust
// crates/server/src/handlers/shared.rs（改造后）
const PASSTHROUGH_HEADERS: &[&str] = &[
    "anthropic-beta",
    "anthropic-version",
    "user-agent",
];
```

### 2. 客户端侧设置合理的 UA

比如 maestro 自己的 reqwest client：

```rust
reqwest::Client::builder()
    .user_agent("maestro/0.1")
    .build()
```

### 3. Provider 侧兜底：通过 model.extra_headers 强制 UA

对 cursorlinkai 这种挑剔的 provider，在 admin UI 上给对应 model 加：

```
user-agent: curl/8.7.1
```

`passthrough.rs::build_upstream_request` 会把 `config.extra_headers` 里的条目设到请求里，覆盖 reqwest 默认 UA（前提：客户端没发 UA 或 merge 策略允许）。

## 六、merge_headers 优先级与陷阱 ⚠️

参见 `provider_builder.rs::merge_headers`：

```rust
// 默认：客户端 header 覆盖 model.extra_headers
// 例外：anthropic-beta 走"值合并 + 去重"
```

这意味着：

| maestro 发的 UA | model.extra_headers 的 UA | 上游实际收到 |
|-----------------|---------------------------|-------------|
| 未设 | `curl/8.7.1` | `curl/8.7.1` |
| `maestro/0.1` | `curl/8.7.1` | **`maestro/0.1`**（客户端赢） |
| `maestro/0.1` | 未设 | `maestro/0.1` |

**陷阱**：如果客户端发了一个恰好在 provider 黑名单里的 UA，model.extra_headers 里设的"兜底 UA"**不会生效**。这符合"透明代理"默认语义，但对 cursorlinkai 这类靠 UA 绕怪癖的场景不友好。

## 七、待办/将来考虑

1. **支持 per-provider force-override 语义**
   需求：某些 provider 需要"不管客户端发什么，都强制替换成某个 UA"。当前架构下可以通过：
   - 在 `ProviderConfig` 或 `Model` 上加一个 `force_headers: HashMap<String, String>`
   - 在 `merge_headers` 之后、`build_upstream_request` 发送前再覆盖一次
   - 或者用 `!` / `force:` 前缀在 extra_headers 的 key 上表达（`force:user-agent`）

2. **结构化记录 provider 的 UA 要求**
   不同 provider 对 UA 敏感度不一（官方 Anthropic/OpenAI 几乎不管；cursorlinkai 这类转卖严格）。可以考虑在 provider 配置里加个 `ua_policy` 枚举：`passthrough | force | sdk_like`。

3. **自动健康探测**
   定期用一个最小请求（带标准 UA）探测 provider 是否正常，若连续失败自动降权。当前的 circuit-breaker 已经是"失败后降权"，可以结合 UA 问题做更细的分类。

## 八、教训

- **透明代理**应当默认透传常见的 identifying headers，不要只挑 API 语义相关的（如 `anthropic-*`）。
- 第三方中转 provider 的行为不可预期、不透明，排查此类问题**先用 curl 直连复现，再做同等条件的 UA/header 变参对比**。
- 当"curl 可用 / SDK 不可用"同时成立时，第一嫌疑就是 UA（或 reqwest vs libcurl 的 HTTP 行为差异）。
