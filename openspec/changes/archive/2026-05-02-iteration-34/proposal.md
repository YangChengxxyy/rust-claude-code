## Why

The current codebase has all core agent logic (QueryLoop, HookRunner, CompactionService, system prompt builder) tightly coupled inside `crates/cli/`, making it impossible to embed the agent loop as a library or support third-party extensions. With 33 iterations complete and all major features implemented, the final iteration addresses the last architectural gap: extract a reusable SDK crate and build a plugin system to make the Rust implementation extensible beyond the CLI.

## What Changes

**Phase A — SDK Foundation (34a):**

- **BREAKING**: Create new `crates/sdk/` crate with zero dependencies on `tui/` or `cli/`
- Move `QueryLoop` from `cli/` to `sdk/`, refactored to use abstract traits (`OutputSink`, `PermissionUI`, `UserQuestionUI`) instead of the concrete `TuiBridge` type
- Move `HookRunner` from `cli/` to `sdk/` (already self-contained, depends only on `core`)
- Move `CompactionService` from `cli/` to `sdk/`
- Move `system_prompt` builder from `cli/` to `sdk/`
- Define `OutputSink`, `PermissionUI`, `UserQuestionUI` traits in `sdk/`
- Implement the new traits on `TuiBridge` in `tui/`
- Add `SessionBuilder` API: `Session::builder().client(c).tools(t).build()?.send(prompt).await`
- Adapt `cli/main.rs` to use the SDK API instead of constructing `QueryLoop` directly

**Phase B — Plugin System (34b):**

- Define `PluginManifest` JSON format (`plugin.json`) for declaring MCP servers, custom agents, and slash commands
- Plugin discovery from `~/.claude/plugins/` and `.claude/plugins/`
- Plugin loading: parse manifests, start MCP servers, register agents and slash commands
- Dynamic `SlashCommand` trait with runtime registration (refactor from static `SLASH_COMMANDS` const + `match` dispatch)
- `/plugin install <path>`, `/plugin list`, `/plugin remove <name>` commands
- `/reload-plugins` command for hot-reload
- Plugin lifecycle management (init, reload, shutdown with cleanup)

## Capabilities

### New Capabilities

- `sdk-foundation`: Extract the agent loop and supporting services into a standalone `sdk/` crate with trait-based abstractions (OutputSink, PermissionUI, UserQuestionUI) and a SessionBuilder API, enabling headless embedding of the agent runtime without TUI or CLI dependencies.
- `plugin-system`: Define plugin manifest format, discovery from user and project plugin directories, dynamic loading of MCP servers/custom agents/slash commands, runtime slash command registration via trait-based registry, and plugin lifecycle management commands (`/plugin install/list/remove`, `/reload-plugins`).

### Modified Capabilities

- `slash-command-extensions`: Requirement change — slash commands transition from compile-time static registration (`const SLASH_COMMANDS` array + `match` dispatch) to runtime dynamic registration via a `SlashCommand` trait and `SlashCommandRegistry`. Existing commands remain available with identical behavior; the change is to the registration mechanism.

## Impact

- **New crate**: `crates/sdk/` — depends on `core`, `api`, `tools` (not `tui` or `cli`)
- **Major refactor**: `crates/cli/src/query_loop.rs` (2003 lines) → `crates/sdk/src/agent_loop.rs`, replaces ~50 `bridge` field accesses with trait calls
- **Moved**: `crates/cli/src/hooks.rs` → `crates/sdk/src/hooks.rs`, `crates/cli/src/compaction.rs` → `crates/sdk/src/compaction.rs`, `crates/cli/src/system_prompt.rs` → `crates/sdk/src/system_prompt.rs`
- **Modified**: `crates/tui/src/bridge.rs` — implement `OutputSink`/`PermissionUI`/`UserQuestionUI` traits on `TuiBridge`
- **Modified**: `crates/cli/src/main.rs` — use `Session::builder()` API instead of direct `QueryLoop` construction
- **Modified**: `crates/tui/src/app.rs` — refactor static slash command dispatch to dynamic trait-based registry (34b)
- **New types**: `OutputSink` trait, `PermissionUI` trait, `UserQuestionUI` trait, `SessionBuilder`, `PluginManifest`, `SlashCommand` trait, `SlashCommandRegistry`
- **Existing specs updated**: `slash-command-extensions/spec.md` (new requirements for dynamic registration)
