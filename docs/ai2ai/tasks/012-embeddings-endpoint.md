# 任务 012: /v1/embeddings 端点实现

> 状态：✅ 已关闭
> 分支：main（直接提交）
> 配对迭代：[iterations/012-embeddings-endpoint.md](../iterations/012-embeddings-endpoint.md)

## 迭代目标
补全 Proxy 的 embedding 端点，支持 OpenAI 兼容的 /v1/embeddings 请求代理。

## 设计摘要

### 核心决策
- 直接透传 JSON，不经 lortex 内部格式转换（Embedding 只有 OpenAI 格式，无需转换层）
- 复用现有 fallback、限流、熔断、usage tracking 机制

### 不做
- Anthropic 格式 embedding（Anthropic 不支持）
- Streaming embedding（OpenAI embedding API 不支持）
- 前端/Admin 变更（embedding 模型用现有 Models 页面创建即可）

## 验收标准
- POST /v1/embeddings 端点可用
- 支持 string | string[] input
- encoding_format / dimensions 透传
- 模型类型校验（非 embedding 模型返回 400）
- Usage 记录 + Credit 扣减
- Fallback + 模型级限流
- `cargo test --workspace` 全量通过

## 任务分解
- [x] T1: lortex_core — 扩展 Provider trait embed 签名
  - 新增 EmbeddingRequest / EmbeddingResponse / EmbeddingData / EmbeddingUsage
  - 验证：4 个 serde 测试通过
- [x] T2: OpenAI provider — 实现 embed 方法
  - 验证：编译通过
- [x] T3: Server proto — 新增 Embedding 类型
  - EmbeddingRequest（EmbeddingInput untagged enum）/ EmbeddingResponse
  - 验证：4 个 serde 测试通过
- [x] T4: Handler + 路由 — embed.rs + 注册
  - 验证：编译通过，路由注册
- [x] T5: 全量测试
  - cargo test --workspace: 356 tests passed, 0 failed, 0 warnings
