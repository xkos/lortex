# 迭代 004a: 日志 + Admin URL 规范化

> 分支：iter/004a-logging
> 日期：2026-04-11

## 目标

为 proxy 添加结构化日志，规范化 Admin URL 路径为 `/admin/api/v1`（为 004b Admin Web 的 `/admin/web` 做准备）。

## 完成内容

| 模块 | 说明 |
|------|------|
| routes | tower-http TraceLayer（HTTP access log：method、path、status、耗时） |
| middleware/proxy_auth | 鉴权日志（key disabled、credit exceeded、authenticated） |
| handlers/chat | 路由日志（requested_model → resolved_model）、上游调用耗时、token 用量、credit 扣减 |
| handlers/messages | 同上，Anthropic 端点 |
| routes + tests | Admin URL 从 `/admin/v1` 改为 `/admin/api/v1`，20 处引用更新 |

无新增测试（日志不改变行为），288 tests 全部通过。

## 回归影响

- Admin URL 路径变更：`/admin/v1/*` → `/admin/api/v1/*`（breaking change，但尚未发布）
- 新增 tower-http TraceLayer，所有 HTTP 请求自动记录 access log
