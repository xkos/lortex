//! lortex-memory: 记忆实现
//!
//! 提供 `Memory` trait 的具体实现：
//! - [`InMemoryStore`] — 按 session 存储全部消息的内存存储
//! - [`SlidingWindowMemory`] — 只保留最近 N 条消息的滑动窗口

pub mod in_memory;
pub mod sliding_window;

pub use in_memory::InMemoryStore;
pub use sliding_window::SlidingWindowMemory;
