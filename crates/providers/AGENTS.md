# providers — LLM 提供商

## 职责

实现 core 中定义的 `Provider` trait，适配各种 LLM 后端。

## 依赖

- `core` — Provider trait、Message、ToolCall 等类型

## 已有实现

- **OpenAIProvider**（openai.rs）— GPT 系列，支持 complete 和 complete_stream，可配置 base_url
- **AnthropicProvider**（anthropic.rs）— Claude 系列，system 作为顶层参数

## 计划实现（Phase 2）

- **GeminiProvider**（gemini.rs）— Google Gemini
- **DeepSeekProvider**（deepseek.rs）— DeepSeek
- **LocalProvider**（local.rs）— 本地模型（Ollama / llama.cpp）
- 兼容 OpenAI API 格式的第三方服务可直接用 OpenAIProvider + 自定义 base_url

## 编码规范

- 每个 Provider 一个文件
- 通过 feature flag 控制编译（如 `features = ["openai", "anthropic"]`）
- 流式输出当前为简化版（一次性解析再发出），后续需改为真正的流式解析
- API key 等敏感信息通过构造函数传入，不读取环境变量（让调用方决定配置来源）
