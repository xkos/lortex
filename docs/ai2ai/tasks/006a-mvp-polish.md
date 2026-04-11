# 任务 006a: MVP 补齐

> 状态：✅ 已关闭
> 分支：iter/006a-mvp-polish
> 配对迭代：[iterations/006a-mvp-polish.md](../iterations/006a-mvp-polish.md)

## 迭代目标
补齐 MVP 阶段的粗糙边：缺失的 Model update 端点、extra_headers 注入、resolve_model 去重、cache token 支持。

## 验收标准（人审核/补充）
- Admin API 可 PUT 更新 Model 字段
- Admin Web 有 Model 编辑功能
- extra_headers 注入到上游请求中
- resolve_model / build_provider 从 chat.rs 和 messages.rs 提取到共享模块
- core Usage 支持 cache token 字段，handlers 正确传递
- `cargo test --workspace` 全量通过

## 任务分解
- [x] T1: Model update admin endpoint + Admin Web 编辑
  - 验证：PUT /admin/api/v1/models/{provider_id}/{model_name} 更新成功；Web 可编辑 Model
- [x] T2: extra_headers 注入到 provider 请求
  - 验证：Model 配置的 extra_headers 出现在上游 HTTP 请求头中
- [x] T3: resolve_model + build_provider 抽取到共享模块
  - 验证：chat.rs 和 messages.rs 不再有重复的 resolve/build 函数
- [x] T4: core Usage 扩展 cache token 字段 + handler 传递
  - 验证：cache_write/read tokens 从上游响应提取并传给 deduct_credits
- [x] T5: 测试验证
  - 验证：cargo test --workspace 全量通过（289 tests）
