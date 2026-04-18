# 测试 Checklist

> AI 生成和维护，人审核和勾选。
> 当前迭代：015-admin-ui-merge + 016-model-level-circuit-breaker

## 本迭代新增

### 015 — 供应商 & 模型合并页面

- [x] 侧边栏：只显示 3 项（供应商 & 模型、API 密钥、用量统计），不再有独立的"模型"入口
- [x] 折叠面板：每个供应商显示为可展开的 `el-collapse` 面板
- [x] 面板 header：显示供应商名称、vendor 标签、启用状态、模型数量、编辑/删除按钮
- [x] 展开箭头：在左侧（而非默认右侧）
- [x] 整栏可点击：点击 header 任意位置（编辑/删除按钮除外）均可展开/收起
- [x] 模型表格：展开后显示该供应商下所有模型，含名称、类型、API 格式、能力、状态列
- [x] 添加模型：每个供应商面板内的"添加模型"按钮自动绑定 provider_id，无需手动选择供应商
- [x] 供应商 CRUD：创建/编辑/删除供应商正常工作
- [x] 模型 CRUD：创建/编辑/删除模型正常工作
- [x] 模型默认能力：新建模型默认勾选 流式输出、工具、缓存、结构化输出、预填充
- [x] i18n：中英文切换后 "供应商 & 模型" / "Providers & Models" 等文本正确

### 016 — 熔断器从 Provider 级别改为 Model 级别

- [x] 熔断粒度：熔断 key 从 `provider_id` 改为 `provider_id/vendor_model_name`
- [x] 独立熔断：单个模型熔断不影响同 provider 下其他模型
- [x] 健康列：模型表格中新增"健康"列，每行独立显示 正常/已熔断/半开 状态
- [x] 重置按钮：熔断状态下显示重置按钮，点击可重置单个模型
- [x] Admin API：`POST /health/{provider_id}/{model_name}/reset` 路径正确工作
- [x] Fallback：模型熔断后，请求自动 fallback 到其他可用模型
- [x] 全量测试：`cargo test --workspace` 通过

## 回归测试

- [x] server: `cargo test -p lortex-server` → unit + integration tests passed
- [x] proxy API: `cargo test -p lortex-server --test proxy_api` → passed
- [x] admin API: `cargo test -p lortex-server --test admin_api` → passed
- [x] /v1/chat/completions：正常工作不受影响
- [x] /v1/messages：正常工作不受影响
- [x] /v1/embeddings：正常工作不受影响
- [x] RPM/TPM per-ApiKey 限流：正常工作
- [x] 模型级限流 + 溢出降级：正常工作
- [x] Usage Dashboard：正常工作
- [x] ApiKeys — Model Mapping：正常工作
