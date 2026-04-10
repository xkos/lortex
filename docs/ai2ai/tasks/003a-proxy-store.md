# 任务 003a: Proxy 存储层 + 管理 API

> 状态：✅ 已关闭
> 分支：iter/003a-proxy-store
> 配对迭代：[iterations/003a-proxy-store.md](../iterations/003a-proxy-store.md)

## 迭代目标
搭建 server crate 骨架，实现数据模型、SQLite 存储和 Admin CRUD API，让 proxy binary 能启动并管理 Provider/Model/ApiKey 数据。

## 验收标准（人审核/补充）
- server crate 结构清晰，数据模型完整（Provider/Model/ApiKey）
- ProxyStore trait 定义完整，SQLite 实现可用
- SQLite migration 自动建表，schema 包含所有预留字段（多模态、缓存等）
- Admin API 可 CRUD Provider/Model/ApiKey，含 reset-credits
- proxy binary 可启动，监听端口，admin API 可用
- 单元测试覆盖 store 层和数据模型
- `cargo test --workspace` 全量通过

## 任务分解
- [x] T1: server crate 骨架 + 数据模型（Provider/Model/ApiKey/Vendor/ModelType）
  - 验证：cargo build 通过，数据模型的构造和序列化正确
- [x] T2: ProxyStore trait + SQLite 实现 + migration
  - 验证：CRUD 全路径测试通过，migration 自动建表
- [x] T3: Admin API handlers（Provider/Model/ApiKey CRUD + reset-credits）
  - 验证：通过 HTTP 请求可完成所有管理操作
- [x] T4: proxy binary 入口 + 启动配置
  - 验证：`cargo run --bin lortex-proxy` 可启动，admin API 可访问
- [x] T5: 单元测试 + 集成测试
  - 验证：store 层全路径测试，admin API 端到端测试
