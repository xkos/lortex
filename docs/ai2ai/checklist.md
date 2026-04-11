# 测试 Checklist

> AI 生成和维护，人审核和勾选。
> 当前迭代：006a-mvp-polish

## 本迭代新增

- [ ] Model Update API: PUT /admin/api/v1/models/{provider}/{name} 可部分更新 Model
- [ ] Admin Web 编辑: Model 列表中可编辑现有 Model
- [ ] extra_headers 注入: Model 配置的 extra_headers 出现在上游请求头
- [ ] resolve_model 去重: chat.rs 和 messages.rs 使用 shared 模块
- [ ] cache token: Usage 包含 cache_creation/read_input_tokens，deduct_credits 传递真实值
- [ ] 全量: `cargo test --workspace` → 289 tests passed, 0 failed

## 回归测试

- [ ] server: `cargo test -p lortex-server` → 57 unit + 21 integration tests passed
- [ ] proxy API: `cargo test -p lortex-server --test proxy_api` → 15 tests passed
- [ ] admin API: `cargo test -p lortex-server --test admin_api` → 6 tests passed
- [ ] core + router + others: 全部通过
