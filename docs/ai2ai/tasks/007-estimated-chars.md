# 任务 007: 请求字符数估算（estimated_chars）

> 状态：✅ 已关闭
> 分支：iter/007-estimated-chars
> 配对迭代：[iterations/007-estimated-chars.md](../iterations/007-estimated-chars.md)

## 迭代目标
在 UsageRecord 中记录 proxy 本地计算的请求字符数，用于与上游 input_tokens 对比检测中转商异常计费。

## 背景
用户通过 OpenAI 格式中转商调用 Anthropic 模型，发现 input_tokens = 62,159，但实际请求内容极少。原因是中转商注入大量 system prompt。通过 estimated_chars vs input_tokens 的量级对比（通常 1 token ≈ 3-4 chars）可识别此类异常。

## 验收标准（人审核/补充）
- UsageRecord 新增 estimated_chars 字段，旧数据向后兼容（显示 `-`）
- 4 条 handler 路径（chat blocking/streaming、messages blocking/streaming）均计算并传入 estimated_chars
- Usage 页面新增 Est.Chars 列
- `cargo test --workspace` 全量通过

## 任务分解
- [x] T1: UsageRecord 模型 + deduct_credits 签名扩展
  - 验证：编译通过，现有测试不受影响 ✅
- [x] T3: Handler 集成 — 4 条路径计算 estimated_chars 并传入 deduct_credits
  - 验证：cargo check 通过，grep 确认 4 处均使用 estimated_chars ✅
- [x] T4: Admin Web — Usage.vue 新增 Est.Chars 列
  - 验证：npm run build 成功 ✅
- [x] T5: 全量测试验证
  - 验证：cargo test --workspace 324 tests passed, 0 failed; clippy 无新增 warning ✅
