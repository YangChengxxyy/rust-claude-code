# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

A Rust reimplementation of Claude Code, built as a Cargo workspace with 5 crates. The project follows the design of the original TypeScript Claude Code but is implemented from scratch in Rust with async-first patterns.

## Build & Test Commands

```bash
# Build the entire workspace
cargo build

# Build and run the CLI binary (named `rust-claude`)
# With a prompt: runs the non-interactive query loop
cargo run -p rust-claude-cli -- "your prompt here"

# Without a prompt: enters the basic TUI
cargo run -p rust-claude-cli

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
cargo check -p rust-claude-cli

# Integration tests (ignored by default, require ANTHROPIC_API_KEY or local endpoint)
cargo test -p rust-claude-api -- --ignored
cargo test -p rust-claude-cli -- --ignored
```

## Environment Variables

- `ANTHROPIC_API_KEY` — API key for x-api-key authentication
- `ANTHROPIC_AUTH_TOKEN` — Auth token for Bearer authentication (used by Claude Code)
- `ANTHROPIC_MODEL` — Override the model (also read from `~/.claude/settings.json` env)
- `ANTHROPIC_BASE_URL` — Custom API endpoint URL
- `CLAUDE_CONFIG_DIR` — Override Claude config directory (default: `~/.claude`)
- `RUST_CLAUDE_MODEL_OVERRIDE` — Override the model (highest priority)
- `RUST_CLAUDE_BASE_URL` — Custom API endpoint URL (highest priority)
- `RUST_CLAUDE_BEARER_AUTH` — Set to `1` or `true` to use Bearer auth instead of x-api-key header
- `RUST_CLAUDE_STREAM` — Set to `1`/`true` or `0`/`false` to override streaming

### Authentication

Credentials are resolved in this order:
1. Config file `api_key` field → uses x-api-key header
2. `ANTHROPIC_API_KEY` env → uses x-api-key header
3. `ANTHROPIC_AUTH_TOKEN` env → uses Bearer auth (auto-detected)
4. `apiKeyHelper` from settings.json → runs script, uses output as Bearer token

This means rust-claude works out-of-the-box in a configured Claude Code environment — it reads `~/.claude/settings.json` env variables (including `ANTHROPIC_AUTH_TOKEN` and `ANTHROPIC_BASE_URL`) and auto-selects the correct auth mode.

Config file location: `~/.config/rust-claude-code/config.json`
Claude Code settings: `~/.claude/settings.json` (reads `env`, `model`, `apiKeyHelper` fields)
Permission rules location: `~/.config/rust-claude-code/permissions.json`

## CLI Parameters

```
rust-claude [OPTIONS] [PROMPT]...

Options:
  -m, --mode <MODE>                  Permission mode: default, accept-edits, bypass, plan, dont-ask
      --model <MODEL>                Model to use
  -p, --print                        Print response and exit (non-interactive mode)
      --output-format <FORMAT>       Output format: text, json
      --max-turns <N>                Maximum agentic turns
      --system-prompt <TEXT>         Override system prompt
      --system-prompt-file <PATH>    Read system prompt from file
      --append-system-prompt <TEXT>  Append to system prompt
      --append-system-prompt-file <PATH>  Append system prompt from file
      --allowed-tools <TOOLS>        Comma-separated tool allowlist
      --disallowed-tools <TOOLS>     Comma-separated tool denylist
      --max-tokens <N>               Maximum output tokens
      --no-stream                    Disable streaming
      --verbose                      Verbose mode
      --settings <PATH>              Path to Claude Code settings.json
```

### Configuration Priority Chain

For each value, the resolution order is:

| Value | Priority (high → low) |
|---|---|
| model | `RUST_CLAUDE_MODEL_OVERRIDE` → `--model` → `ANTHROPIC_MODEL` → `settings.json model` → config.json → default |
| credential | config.json `api_key` → `ANTHROPIC_API_KEY` → `ANTHROPIC_AUTH_TOKEN` → `apiKeyHelper` |
| base_url | `RUST_CLAUDE_BASE_URL` → `ANTHROPIC_BASE_URL` → config.json → API default |
| bearer_auth | `RUST_CLAUDE_BEARER_AUTH` → auto (true if `ANTHROPIC_AUTH_TOKEN`) → config.json → `false` |
| stream | `RUST_CLAUDE_STREAM` → `--no-stream` → config.json → `true` |
| max_tokens | `--max-tokens` → config.json → `16384` |
| system_prompt | `--system-prompt`/`--system-prompt-file` → config.json → `None` |
| permission_mode | `--mode` → config.json → `Default` |
| max_turns | `--max-turns` → `8` (QueryLoop default) |

## Architecture

### Workspace Crates (dependency order: core → api/tools → cli/tui)

- **`core`** — Shared types with zero external service dependencies. Defines `Message`, `ContentBlock`, `AppState`, `PermissionMode`/`PermissionRule`/`PermissionCheck`, `PermissionManager`, `Config` (with `base_url`, `bearer_auth`, and `ANTHROPIC_AUTH_TOKEN` auto-detection), `ClaudeSettings` (reads `~/.claude/settings.json` — `env`, `model`, `apiKeyHelper` fields), `ToolInfo`/`ToolResult`. Everything else depends on this crate.
- **`api`** — Anthropic HTTP client with SSE streaming. `AnthropicClient` sends requests and returns `MessageStream` (a `Pin<Box<dyn Stream<Item=StreamEvent>>>`). The streaming module has delta accumulators (`TextDeltaAccumulator`, `ThinkingDeltaAccumulator`, `ToolUseDeltaAccumulator`) that assemble partial events into complete content blocks.
- **`tools`** — Tool trait and implementations: `BashTool`, `FileReadTool`, `FileEditTool`, `FileWriteTool`, `TodoWriteTool`. Each implements the `Tool` trait (`info()`, `is_read_only()`, `is_concurrency_safe()`, `execute()`). `ToolRegistry` indexes tools by name, partitions them by concurrency safety for execution, and supports allow/deny list filtering via `apply_tool_filters()`.
- **`cli`** — Binary crate (`rust-claude`) and `QueryLoop` — the agentic loop that sends messages, streams responses, detects `tool_use` stop reasons, checks permissions, executes tools (concurrent-safe in parallel via `join_all`, others serially), appends `tool_result` messages, and loops up to `max_rounds` (default 8). CLI supports extensive parameters (`--model`, `--mode`, `--print`, `--max-turns`, `--system-prompt`, etc.) and a unified priority chain for configuration resolution. With a prompt argument it runs non-interactively; without a prompt it enters the basic TUI and dispatches prompts through the same `QueryLoop` in a background task.
- **`tui`** — Terminal UI crate with a basic ratatui foundation: `App` state object, `ChatMessage` / `AppEvent` model, rendering layer (`ui.rs`), bridge for query-loop events (`TuiBridge`), and `TerminalGuard` for raw mode + alternate screen lifecycle. The current CLI integration is minimal: it submits prompts from the TUI, runs `QueryLoop`, and pushes final assistant text plus usage updates back into the UI. Token-by-token streaming and tool event bridging are not yet connected.

### Key Patterns

- **`ModelClient` trait** (in `cli/query_loop.rs`) — Abstracts the API client so `QueryLoop` can be tested with `MockClient` without network calls.
- **`Tool` trait** (in `tools/tool.rs`) — Unified interface for all tools. `ToolContext` carries `tool_use_id` and an `Arc<Mutex<AppState>>`.
- **Permission system** (in `core/permission.rs`) — 5 modes (Default, AcceptEdits, BypassPermissions, Plan, DontAsk). Rules are checked in order: deny rules → allow rules → mode-specific default. `PermissionManager` persists compact rules like `Bash(git *)`. Plan mode blocks all non-read-only operations even if explicitly allowed. QueryLoop currently auto-denies `NeedsConfirmation` because interactive confirmation UI is not yet connected.
- **Content block accumulation** — Streaming responses are assembled from SSE delta events into complete `ContentBlock` values using typed accumulators.
- **TUI event bridge** — The TUI uses an internal `AppEvent` channel so CLI/query-loop integration can feed messages into the UI without coupling rendering to transport. At the moment, the bridge is only used for final assistant text, errors, and usage updates; token streaming and tool-level progress are future work.

### Tool Concurrency Model

Tools declare `is_concurrency_safe()`. During tool execution in the query loop:
1. Concurrent-safe tools are collected and run in parallel (`join_all`)
2. Non-concurrent-safe tools run serially afterward

Currently only `FileReadTool` is marked as concurrency-safe.

## Language

The requirements doc (`doc/requirement.md`) and design notes (`doc/iteration-3-alignment.md`) are written in Chinese. Code, comments, and identifiers are in English.
