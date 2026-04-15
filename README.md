# Lortex

A modular, high-performance **Rust Agent framework** and **LLM Proxy service**.

Lortex serves two roles:
- **Library** -- Build AI agents, multi-agent systems, and tool-augmented workflows in Rust.
- **Binary (`lortex-proxy`)** -- A ready-to-run local LLM proxy that aggregates multiple providers behind a unified API, with routing, rate limiting, usage tracking, and an admin dashboard.

## Features

### Agent Framework

- **Provider-agnostic** -- Business logic via traits, not tied to any specific LLM
- **Heterogeneous model routing** -- Route tasks to different models by quality, cost, or capability
- **Multi-agent orchestration** -- Pipeline, parallel, and hierarchical agent patterns
- **Protocol support** -- MCP and A2A as first-class citizens
- **Tool system** -- Built-in tools + `#[tool]` proc macro for custom tools
- **Guardrails** -- Content filtering, token budget, rate limiting, tool approval
- **Memory** -- In-memory and sliding window implementations

### LLM Proxy (`lortex-proxy`)

- **Multi-provider** -- OpenAI, Anthropic, and compatible APIs behind one endpoint
- **API key management** -- Per-key model access, credit limits, RPM/TPM rate limiting
- **Smart routing** -- Fallback chains with circuit breaker protection
- **Usage tracking** -- Token counting, credit accounting, and dashboard with charts
- **Protocol translation** -- Automatic OpenAI <-> Anthropic format conversion
- **Admin Web UI** -- Vue 3 + Element Plus management console (embedded in binary)
- **Streaming** -- Full SSE streaming support for all providers

## Architecture

```
lortex (facade crate)
  |
  +-- core          Core traits: Agent, Tool, Provider, Memory, Message
  +-- executor      Execution engine: Runner, ReAct, PlanAndExecute
  +-- providers     LLM providers: OpenAI, Anthropic, etc.
  +-- router        Heterogeneous model routing
  +-- protocols     Agent protocols: MCP, A2A
  +-- tools         Built-in tools + tool registry
  +-- swarm         Multi-agent orchestration
  +-- guardrails    Safety: content filter, rate limiter, token budget
  +-- memory        Memory implementations
  +-- macros        Proc macros: #[tool]
  +-- server        HTTP proxy service + admin API + web UI
```

Dependency rule: `core` is the only shared dependency. Sub-crates avoid cross-dependencies.

## Quick Start

### As a Library

Add to your `Cargo.toml`:

```toml
[dependencies]
lortex = "0.1"
tokio = { version = "1", features = ["full"] }
```

```rust
use lortex::prelude::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let provider = lortex::providers::openai::OpenAIProvider::new(
        std::env::var("OPENAI_API_KEY")?,
    );

    let agent = Agent::builder()
        .name("assistant")
        .instructions("You are a helpful assistant.")
        .provider(provider)
        .build();

    let result = Runner::new(agent).run("Hello!").await?;
    println!("{}", result);
    Ok(())
}
```

### As a Proxy

```bash
# Build and run
cargo run --bin lortex-proxy -- --config config.toml

# Or with environment variables
export LORTEX_ADMIN_KEY=your-admin-key
export LORTEX_DB_PATH=lortex.db
cargo run --bin lortex-proxy
```

Then configure providers and API keys via the admin UI at `http://localhost:8080/admin/web/`.

Use the proxy as a drop-in replacement for OpenAI or Anthropic APIs:

```bash
# OpenAI-compatible endpoint
curl http://localhost:8080/v1/chat/completions \
  -H "Authorization: Bearer your-proxy-key" \
  -H "Content-Type: application/json" \
  -d '{"model": "gpt-4o", "messages": [{"role": "user", "content": "Hello"}]}'

# Anthropic-compatible endpoint
curl http://localhost:8080/v1/messages \
  -H "x-api-key: your-proxy-key" \
  -H "Content-Type: application/json" \
  -d '{"model": "claude-sonnet-4-20250514", "max_tokens": 1024, "messages": [{"role": "user", "content": "Hello"}]}'
```

## Building

```bash
# Check
cargo check --workspace

# Test
cargo test --workspace

# Build proxy binary
cargo build --release --bin lortex-proxy

# Build admin web UI (requires Node.js)
cd crates/server/admin-web && npm install && npm run build
```

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.
