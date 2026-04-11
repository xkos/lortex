# 任务 004b: Admin Web 管理后台

> 状态：✅ 已关闭
> 分支：iter/004b-admin-web
> 配对迭代：[iterations/004b-admin-web.md](../iterations/004b-admin-web.md)

## 迭代目标
实现 Vue 3 + Element Plus 的 Admin Web 管理后台，嵌入到 proxy binary 中，通过 --with-admin-web 参数开启。

## 验收标准（人审核/补充）
- Vue 3 + Vite + Element Plus 前端项目可构建
- 登录页输入 admin_key 验证
- Provider/Model/ApiKey 管理页面可 CRUD
- 前端构建产物通过 rust-embed 嵌入 binary
- --with-admin-web 参数控制是否启用
- /admin/web/* 路由 serve 静态文件
- `cargo test --workspace` 全量通过

## 任务分解
- [x] T1: 后端静态文件服务（rust-embed + CLI 参数 + /admin/web/* 路由）
  - 验证：/admin/web/ 可返回 HTML
- [x] T2: 前端项目初始化 + 登录页
  - 验证：npm run build 产出静态文件，登录页可验证 admin_key
- [x] T3: Provider/Model/ApiKey 管理页面
  - 验证：三个资源的 CRUD 操作可通过 Web 完成
- [x] T4: 构建集成 + 测试
  - 验证：cargo build 嵌入前端产物，启动后 Web 可访问
