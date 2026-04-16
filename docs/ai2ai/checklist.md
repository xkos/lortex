# 测试 Checklist

> AI 生成和维护，人审核和勾选。
> 当前迭代：014-apikey-model-map

## 本迭代新增

- [x] ApiKeys — Model Mapping section：创建/编辑对话框中 Default Model 下方显示"Model Mapping / 模型映射"section
- [x] ApiKeys — 添加映射：点击 Add Mapping 按钮可新增占位符+目标模型行
- [x] ApiKeys — Quick Add：点击 "Quick Add: Claude Code" 按钮 → 自动填入 claude-sonnet-4-6 / claude-opus-4-6 / claude-haiku-4-5-20251001 三行
- [x] ApiKeys — 保存映射：添加 mapping 后保存，重新编辑 → mapping 值不丢失
- [x] ApiKeys — 删除映射：删除所有 mapping 行后保存 → model_map 为 null/空
- [x] 解析逻辑：配置 model_map 后，请求中使用占位符名（如 claude-sonnet-4-6）→ proxy 解析到 map 配置的目标模型
- [x] 解析逻辑：未命中 model_map 时走正常模型解析路径
- [x] PROXY_MANAGED 优先：model_name="PROXY_MANAGED" 时仍解析到 default_model，不走 model_map
- [x] i18n：中英文切换后 模型映射 / Model Mapping 等新增文本正确显示
- [x] 全量：`cargo test --workspace` → 356 tests passed, 0 failed

## 回归测试

- [x] server: `cargo test -p lortex-server` → unit + integration tests passed
- [x] proxy API: `cargo test -p lortex-server --test proxy_api` → 15 tests passed
- [x] admin API: `cargo test -p lortex-server --test admin_api` → 6 tests passed
- [x] /v1/chat/completions：正常工作不受影响
- [x] /v1/messages：正常工作不受影响
- [x] /v1/embeddings：正常工作不受影响
- [x] RPM/TPM per-ApiKey 限流：正常工作
- [x] 熔断器 + Fallback：正常工作
- [x] 模型级限流 + 溢出降级：正常工作
- [x] Usage Dashboard：缓存命中率 + 原有图表正常
- [x] Models — Custom Headers：编辑不受影响
