# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A Rust reimplementation of Claude Code, built as a Cargo workspace with 5 crates. The project follows the design of the original TypeScript Claude Code but is implemented from scratch in Rust with async-first patterns.

## Build & Test Commands

```bash
# Build the entire workspace
cargo build

# Build and run the CLI binary (named `rust-claude`)
cargo run -p rust-claude-cli -- "your prompt here"

# Run all tests across all crates
cargo test --workspace

# Run tests for a specific crate
cargo test -p rust-claude-core
cargo test -p rust-claude-api
cargo test -p rust-claude-tools
cargo test -p rust-claude-cli
cargo test -p rust-claude-tui

# Run a single test by name
cargo test -p rust-claude-core -- test_name

# Check without building
cargo check --workspace
cargo check -p rust-claude-tui

# Integration tests (ignored by default, require ANTHROPIC_API_KEY or local endpoint)
cargo test -p rust-claude-api -- --ignored
cargo test -p rust-claude-cli -- --ignored
```

## Environment Variables

- `ANTHROPIC_API_KEY` — Required for running the CLI or integration tests
- `RUST_CLAUDE_MODEL_OVERRIDE` — Override the model from config
- `RUST_CLAUDE_BASE_URL` — Custom API endpoint URL
- `RUST_CLAUDE_BEARER_AUTH` — Set to `1` or `true` to use Bearer auth instead of x-api-key header

Config file location: `~/.config/rust-claude-code/config.json`
Permission rules location: `~/.config/rust-claude-code/permissions.json`

## Architecture

### Workspace Crates (dependency order: core → api/tools → cli/tui)

- **`core`** — Shared types with zero external service dependencies. Defines `Message`, `ContentBlock`, `AppState`, `PermissionMode`/`PermissionRule`/`PermissionCheck`, `PermissionManager`, `Config`, `ToolInfo`/`ToolResult`. Everything else depends on this crate.
- **`api`** — Anthropic HTTP client with SSE streaming. `AnthropicClient` sends requests and returns `MessageStream` (a `Pin<Box<dyn Stream<Item=StreamEvent>>>`). The streaming module has delta accumulators (`TextDeltaAccumulator`, `ThinkingDeltaAccumulator`, `ToolUseDeltaAccumulator`) that assemble partial events into complete content blocks.
- **`tools`** — Tool trait and implementations: `BashTool`, `FileReadTool`, `FileEditTool`, `FileWriteTool`, `TodoWriteTool`. Each implements the `Tool` trait (`info()`, `is_read_only()`, `is_concurrency_safe()`, `execute()`). `ToolRegistry` indexes tools by name and partitions them by concurrency safety for execution.
- **`cli`** — Binary crate (`rust-claude`) and `QueryLoop` — the agentic loop that sends messages, streams responses, detects `tool_use` stop reasons, checks permissions, executes tools (concurrent-safe in parallel via `join_all`, others serially), appends `tool_result` messages, and loops up to `max_rounds` (default 8). CLI supports `--mode` / `-m` for permission mode overrides.
- **`tui`** — Terminal UI crate with a basic ratatui foundation: `App` state object, `ChatMessage` / `AppEvent` model, rendering layer (`ui.rs`), bridge for query-loop events (`TuiBridge`), and `TerminalGuard` for raw mode + alternate screen lifecycle. It is not yet wired into the CLI entrypoint.

### Key Patterns

- **`ModelClient` trait** (in `cli/query_loop.rs`) — Abstracts the API client so `QueryLoop` can be tested with `MockClient` without network calls.
- **`Tool` trait** (in `tools/tool.rs`) — Unified interface for all tools. `ToolContext` carries `tool_use_id` and an `Arc<Mutex<AppState>>`.
- **Permission system** (in `core/permission.rs`) — 5 modes (Default, AcceptEdits, BypassPermissions, Plan, DontAsk). Rules are checked in order: deny rules → allow rules → mode-specific default. `PermissionManager` persists compact rules like `Bash(git *)`. Plan mode blocks all non-read-only operations even if explicitly allowed. QueryLoop currently auto-denies `NeedsConfirmation` because interactive confirmation UI is not yet connected.
- **Content block accumulation** — Streaming responses are assembled from SSE delta events into complete `ContentBlock` values using typed accumulators.
- **TUI event bridge** — The TUI uses an internal `AppEvent` channel so future CLI/query-loop integration can stream deltas, tool events, and usage updates into the UI without coupling rendering to transport.

### Tool Concurrency Model

Tools declare `is_concurrency_safe()`. During tool execution in the query loop:
1. Concurrent-safe tools are collected and run in parallel (`join_all`)
2. Non-concurrent-safe tools run serially afterward

Currently only `FileReadTool` is marked as concurrency-safe.

## Language

The requirements doc (`doc/requirement.md`) and design notes (`doc/iteration-3-alignment.md`) are written in Chinese. Code, comments, and identifiers are in English.
