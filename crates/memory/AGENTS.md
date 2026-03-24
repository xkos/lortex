# memory — 记忆实现

## 职责

实现 core 中定义的 `Memory` trait，提供会话存储与检索。

## 依赖

- `core` — Memory trait、Message

## 已有实现

- **InMemoryStore**（in_memory.rs）— 按 session 存储全部消息，支持 after 时间过滤
- **SlidingWindowMemory**（sliding_window.rs）— 每 session 只保留最近 N 条消息

## 待实现

- **SummaryMemory** — 超出窗口的内容自动调用 LLM 摘要（Phase 4）
- **向量记忆** — 基于 Retriever trait 的长期记忆（Phase 4，RAG 能力）

## 注意事项

- 当前 search 实现是简单文本匹配，无语义搜索
- 向量记忆的实现不在 memory 内部，而是通过 core 中定义的 Retriever trait 由外部提供
