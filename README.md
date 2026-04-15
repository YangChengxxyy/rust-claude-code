# rust-claude-code

A Rust reimplementation of [Claude Code](https://claude.ai/code), built from scratch with async-first patterns.

## Features

- Anthropic API client with SSE streaming
- Agentic query loop with multi-turn tool use
- 5 built-in tools: Bash, FileRead, FileEdit, FileWrite, TodoWrite
- Permission system with 5 modes and persistent rules
- Interactive TUI (ratatui) and non-interactive CLI modes
- Claude Code-compatible CLI parameters and configuration priority chain
- Reads `~/.claude/settings.json` env for seamless setup sharing

## Quick Start

```bash
# Build
cargo build

# Set your API key
export ANTHROPIC_API_KEY="sk-ant-..."

# Non-interactive: pass a prompt
cargo run -p rust-claude-cli -- "explain what this project does"

# Interactive TUI: no prompt
cargo run -p rust-claude-cli
```

## Configuration

### Config file (`~/.config/rust-claude-code/config.json`)

```json
{
  "api_key": "sk-ant-...",
  "model": "claude-sonnet-4-20250514",
  "base_url": "https://api.anthropic.com",
  "bearer_auth": false,
  "max_tokens": 16384,
  "stream": true,
  "permission_mode": "Default",
  "always_allow": ["Bash(git *)", "FileRead"],
  "always_deny": []
}
```

Only `api_key` is required. You can also provide it via `ANTHROPIC_API_KEY` env var.

### Claude Code settings (`~/.claude/settings.json`)

The CLI reads the `env` field from `~/.claude/settings.json` and injects environment variables (without overwriting existing ones):

```json
{
  "env": {
    "ANTHROPIC_MODEL": "claude-opus-4-20250514",
    "ANTHROPIC_BASE_URL": "https://your-proxy.example.com/v1"
  }
}
```

### Priority Chain

Configuration is resolved through a unified priority chain (highest to lowest):

| Value | Priority |
|---|---|
| model | `RUST_CLAUDE_MODEL_OVERRIDE` → `--model` → `ANTHROPIC_MODEL` → config → default |
| base_url | `RUST_CLAUDE_BASE_URL` → `ANTHROPIC_BASE_URL` → config → API default |
| stream | `RUST_CLAUDE_STREAM` → `--no-stream` → config → `true` |
| max_tokens | `--max-tokens` → config → `16384` |
| system_prompt | `--system-prompt` / `--system-prompt-file` → config → `None` |
| permission_mode | `--mode` → config → `Default` |

## CLI Usage

```
rust-claude [OPTIONS] [PROMPT]...

Options:
  -m, --mode <MODE>                      Permission mode: default, accept-edits, bypass, plan, dont-ask
      --model <MODEL>                    Model to use (e.g. claude-sonnet-4-20250514)
  -p, --print                            Print response and exit (non-interactive)
      --output-format <FORMAT>           Output format: text, json
      --max-turns <N>                    Maximum agentic turns
      --system-prompt <TEXT>             Override system prompt
      --system-prompt-file <PATH>        Read system prompt from file
      --append-system-prompt <TEXT>      Append to system prompt
      --append-system-prompt-file <PATH> Append system prompt from file
      --allowed-tools <TOOLS>            Comma-separated tool allowlist
      --disallowed-tools <TOOLS>         Comma-separated tool denylist
      --max-tokens <N>                   Maximum output tokens
      --no-stream                        Disable streaming
      --verbose                          Verbose mode
      --settings <PATH>                  Path to Claude Code settings.json
```

### Examples

```bash
# Use a specific model
cargo run -p rust-claude-cli -- --model claude-opus-4-20250514 "write a sorting function"

# Limit agentic turns and output JSON
cargo run -p rust-claude-cli -- --max-turns 3 --output-format json "analyze this codebase"

# Disable bash tool, allow only file reads
cargo run -p rust-claude-cli -- --disallowed-tools Bash "review the code"

# Use a custom system prompt
cargo run -p rust-claude-cli -- --system-prompt "You are a Rust expert" "explain lifetimes"

# Use a proxy endpoint
export RUST_CLAUDE_BASE_URL="https://your-proxy.example.com/v1"
cargo run -p rust-claude-cli -- "hello"
```

## Architecture

```
rust-claude-code
├── crates/core    — Shared types: Message, AppState, Config, Permission, ClaudeSettings
├── crates/api     — Anthropic HTTP client with SSE streaming
├── crates/tools   — Tool trait + Bash, FileRead, FileEdit, FileWrite, TodoWrite
├── crates/cli     — Binary + QueryLoop (agentic loop) + CLI arg resolution
└── crates/tui     — Terminal UI (ratatui) with event bridge
```

Dependency order: `core` → `api` / `tools` → `cli` / `tui`

## Development

```bash
# Run all tests
cargo test --workspace

# Run tests for a single crate
cargo test -p rust-claude-core

# Check without building
cargo check --workspace

# Integration tests (require ANTHROPIC_API_KEY)
cargo test -p rust-claude-api -- --ignored
```

## License

MIT
