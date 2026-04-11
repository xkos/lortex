# 迭代 004b: Admin Web 管理后台

> 分支：iter/004b-admin-web
> 日期：2026-04-11

## 目标

实现 Vue 3 + Element Plus 的 Admin Web 管理后台，嵌入到 proxy binary 中。

## 完成内容

| 模块 | 说明 |
|------|------|
| 后端 | rust-embed 嵌入静态文件、`--with-admin-web` CLI 参数、`/admin/web/*` 路由（SPA fallback） |
| 前端框架 | Vue 3 + Vite + TypeScript + Element Plus + Vue Router + Axios |
| 登录页 | 输入 admin_key 验证（调用 /providers 接口验证），sessionStorage 存储 |
| Layout | 侧边栏导航（Providers/Models/API Keys）+ 登出 |
| Providers 页 | 列表 + 创建 + 编辑 + 删除，支持 vendor 选择和 base_url 配置 |
| Models 页 | 列表 + 创建 + 删除，支持能力声明、倍率配置、别名 |
| API Keys 页 | 列表 + 创建（显示完整 key 一次）+ 编辑 + 删除 + credit 重置 |

前端构建产物 ~420KB gzip，通过 rust-embed 嵌入 binary，无需单独部署前端。

## 回归影响

- `app_router` 签名新增 `with_admin_web: bool` 参数
- 新增 `rust-embed`、`mime_guess` 依赖
- 288 tests 全部通过
