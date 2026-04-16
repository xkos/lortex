# 测试 Checklist

> AI 生成和维护，人审核和勾选。
> 当前迭代：012-embeddings-endpoint

## 本迭代新增

- [x] 路由注册：POST /v1/embeddings 端点可访问（未授权返回 401，非 embedding 模型返回 400）
- [x] 单文本输入：`"input": "hello"` → 返回 `{"object":"list","data":[{"object":"embedding","index":0,"embedding":[...]}]}`
- [x] 多文本输入：`"input": ["hello","world"]` → 返回 data 数组长度为 2，index 分别为 0 和 1
- [x] encoding_format 透传：`"encoding_format": "base64"` → 上游返回 base64 字符串而非 float 数组
- [x] dimensions 透传：`"dimensions": 256` → 返回向量维度为 256
- [x] 模型类型校验：用 chat 类型模型请求 /v1/embeddings → 返回 400 错误
- [x] Usage 记录：embedding 请求后 Usage 表有对应记录（endpoint="/v1/embeddings", input_tokens>0, output_tokens=0）
- [x] Credit 扣减：embedding 请求后 ApiKey credit_used 增加
- [x] Fallback：主 embedding 模型不可用时自动降级到 fallback embedding 模型
- [x] 模型级限流：配置 rpm_limit 的 embedding 模型超限后跳过，降级到其他 embedding 模型
- [x] 全量：`cargo test --workspace` → 356 tests passed, 0 failed

## 回归测试

- [x] server: `cargo test -p lortex-server` → unit + integration tests passed
- [x] proxy API: `cargo test -p lortex-server --test proxy_api` → 15 tests passed
- [x] admin API: `cargo test -p lortex-server --test admin_api` → 6 tests passed
- [x] /v1/chat/completions：正常工作不受影响
- [x] /v1/messages：正常工作不受影响
- [x] RPM/TPM per-ApiKey 限流：正常工作（009 功能不受影响）
- [x] 熔断器 + Fallback：正常工作（006b 功能不受影响）
- [x] 模型级限流 + 溢出降级：正常工作（011 功能不受影响）
- [x] Usage Dashboard：图表正常（010 功能不受影响）
