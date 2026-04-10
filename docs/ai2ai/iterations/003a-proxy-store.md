# 迭代 003a: Proxy 存储层 + 管理 API

> 分支：iter/003a-proxy-store
> 日期：2026-04-10

## 目标

搭建 server crate 骨架，实现数据模型、SQLite 存储和 Admin CRUD API，让 proxy binary 能启动并管理数据。

## 完成内容

| 模块 | 新增测试数 | 覆盖内容 |
|------|-----------|---------|
| store/sqlite | 15 | Provider/Model/ApiKey 全路径 CRUD、upsert 更新、别名查找、可选 multiplier、extra_headers、credit 操作、migration 幂等 |
| admin API (integration) | 5 | auth 鉴权拒绝、Provider CRUD、Model CRUD、ApiKey CRUD + reset-credits |

新增 server crate，包含：
- 数据模型：Provider（Vendor 枚举）、Model（ModelType 枚举 + 完整能力声明 + 多模态/缓存计费倍率 + extra_headers）、ApiKey（模型组 + credit 额度）
- ProxyStore trait + SQLite 实现（sqlx，:memory: 测试）
- SQLite migration（4 表 + 3 索引，含 usage_records 预留）
- Admin API（axum handlers，Provider/Model/ApiKey CRUD + reset-credits）
- admin_auth 中间件（Bearer token 鉴权）
- lortex-proxy binary（clap CLI，合并/分离端口，SQLite 初始化）

Workspace 共 236 tests 全部通过。

## 未完成 / 遗留

- 003b：代理核心（/v1/chat/completions、/v1/messages、协议转换、credit 扣减）

## 回归影响

无。新增 server crate，未修改任何现有 crate。workspace 根 Cargo.toml 仅修改 tracing-subscriber 添加 env-filter feature。
