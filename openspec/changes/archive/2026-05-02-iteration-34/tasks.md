## 1. SDK Crate Setup

- [x] 1.1 Create `crates/sdk/` directory with `Cargo.toml` depending on `core`, `api`, `tools`, `mcp` only (no `tui` or `cli`)
- [x] 1.2 Add `rust-claude-sdk` to workspace `Cargo.toml` members
- [x] 1.3 Setup `crates/sdk/src/lib.rs` with module declarations

## 2. Define Output Traits

- [x] 2.1 Create `crates/sdk/src/output.rs` with `OutputSink` trait (methods: text_delta, thinking_start, thinking_delta, thinking_complete, tool_input_start, tool_input_delta, tool_use, tool_result, usage, error, todo_update)
- [x] 2.2 Define `PermissionUI` trait (async request method returning PermissionDecision)
- [x] 2.3 Define `UserQuestionUI` trait (async ask method returning Option<AskUserQuestionResponse>)
- [x] 2.4 Add no-op implementations of all three traits for headless mode

## 3. Extract HookRunner

- [x] 3.1 Move `crates/cli/src/hooks.rs` to `crates/sdk/src/hooks.rs`
- [x] 3.2 Update module references in `cli/src/lib.rs` to re-export from sdk (keep backward compat)
- [x] 3.3 Verify: `cargo test -p rust-claude-sdk` passes

## 4. Extract CompactionService

- [x] 4.1 Move `crates/cli/src/compaction.rs` to `crates/sdk/src/compaction.rs`
- [x] 4.2 Update `cli/src/lib.rs` re-exports
- [x] 4.3 Verify: `cargo test -p rust-claude-sdk` passes

## 5. Extract System Prompt Builder

- [x] 5.1 Move `crates/cli/src/system_prompt.rs` to `crates/sdk/src/system_prompt.rs`
- [x] 5.2 Update `cli/lib.rs` re-exports
- [x] 5.3 Mark CORE_PROMPT as `pub(crate)`
- [x] 5.4 Verify: `cargo test -p rust-claude-sdk` passes

## 6. Refactor and Extract QueryLoop

- [x] 6.1 Move `crates/cli/src/query_loop.rs` to `crates/sdk/src/agent_loop.rs`
- [x] 6.2 Replace `bridge: Option<rust_claude_tui::TuiBridge>` field with three optional trait object fields: `output: Option<Box<dyn OutputSink>>`, `permission_ui: Option<Box<dyn PermissionUI>>`, `user_question_ui: Option<Box<dyn UserQuestionUI>>`
- [x] 6.3 Add builder methods: `with_output()`, `with_permission_ui()`, `with_user_question_ui()`
- [x] 6.4 Refactor all ~50 `self.bridge` call sites to use the appropriate trait
- [x] 6.5 Remove `user_question_callback()` method — replaced by `user_question_ui`
- [x] 6.6 Move MockClient and all QueryLoop tests from cli to sdk
- [x] 6.7 Update QueryLoop tests to use trait objects instead of bridge
- [x] 6.8 Verify: all QueryLoop tests in sdk pass

## 7. SessionBuilder API

- [ ] 7.1 Create `crates/sdk/src/session.rs` with `Session<C: ModelClient>` struct wrapping QueryLoop
- [ ] 7.2 Implement `SessionBuilder<C>` with required fields enforced at compile time (client, tools) and optional builder methods
- [ ] 7.3 Implement `Session::send(prompt) -> Result<Message, Error>` method
- [ ] 7.4 Implement `Session::send_streaming(prompt) -> Result<impl Stream<Item = SessionEvent>, Error>` with `SessionEvent` enum
- [ ] 7.5 Add tests for SessionBuilder and Session::send with MockClient
- [ ] 7.6 Verify: `cargo test -p rust-claude-sdk` passes

## 8. Implement SDK Traits on TuiBridge

- [x] 8.1 Add `sdk` dependency to `crates/tui/Cargo.toml`
- [x] 8.2 Implement `OutputSink` for `TuiBridge` by delegating to existing event sending methods
- [x] 8.3 Implement `PermissionUI` for `TuiBridge` by delegating to existing `request_permission()`
- [x] 8.4 Implement `UserQuestionUI` for `TuiBridge` by delegating to existing `request_user_question()`
- [x] 8.5 Verify: `cargo test -p rust-claude-tui` passes

## 9. Adapt CLI to Use SDK

- [x] 9.1 Add `sdk` dependency to `crates/cli/Cargo.toml`
- [x] 9.2 Update `cli/src/lib.rs` to re-export from sdk (hooks, compaction, query_loop, system_prompt)
- [x] 9.3 Refactor `main.rs` QueryLoop construction to use `Session::builder()` API
- [x] 9.4 Refactor `build_agent_context()` to use Session-based sub-agent spawning
- [x] 9.5 Update all `use rust_claude_cli::*` imports across the workspace
- [x] 9.6 Verify: `cargo test --workspace` — all 738+ tests pass
- [ ] 9.7 Smoke test: `cargo run -- "hello"` works correctly in print mode
- [ ] 9.8 Smoke test: TUI launches and accepts prompts

## 10. Plugin Manifest Types

- [x] 10.1 Create `crates/sdk/src/plugin.rs` with `PluginManifest` struct (name, version, description, mcp_servers, custom_agents, slash_commands)
- [x] 10.2 Define `PluginSlashCommand` struct (name, description, prompt template)
- [x] 10.3 Implement `serde::Deserialize` for all plugin types
- [x] 10.4 Add unit tests for manifest parsing (valid manifest, missing fields, invalid JSON)

## 11. Plugin Discovery and Loading

- [x] 11.1 Implement `PluginLoader::discover()` — scan `~/.claude/plugins/` and `.claude/plugins/` for plugin.json
- [x] 11.2 Implement `PluginLoader::load()` — parse manifests, validate, return `Vec<LoadedPlugin>`
- [x] 11.3 Implement project-over-user precedence for same-named plugins
- [x] 11.4 Implement `PluginLoader::unload()` — stop MCP servers, unregister agents/commands
- [x] 11.5 Add tests for discovery (empty dir, multiple plugins, duplicate names)

## 12. Dynamic Slash Command Refactoring

- [x] 12.1 Define `SlashCommand` trait in `crates/tui/` (name, usage, description, execute method)
- [x] 12.2 Implement `SlashCommandRegistry` with `register()`, `unregister()`, `commands()`, `dispatch()`
- [x] 12.3 Extract each existing slash command arm from `handle_slash_command()` into a struct implementing `SlashCommand`
- [x] 12.4 Register all 32 built-in commands in the dynamic registry at TUI startup
- [x] 12.5 Update `handle_slash_command()` to dispatch through the registry
- [x] 12.6 Update slash suggestions to source from the dynamic registry
- [x] 12.7 Verify: all existing slash commands work identically in TUI

## 13. Plugin Slash Command Integration

- [x] 13.1 Implement `PluginSlashCommand::execute()` — submit prompt template (with arg substitution) to agent
- [x] 13.2 Wire plugin slash commands into the dynamic registry when plugins load
- [x] 13.3 Unregister plugin slash commands when plugins unload
- [x] 13.4 Add tests for plugin slash command dispatch

## 14. MCP Dynamic Add/Remove

- [x] 14.1 Add `McpManager::add_server(name, config) -> Result` method
- [x] 14.2 Add `McpManager::remove_server(name) -> Result` method with cleanup
- [x] 14.3 Wire plugin MCP servers into `McpManager::add_server` on plugin load
- [x] 14.4 Wire `McpManager::remove_server` on plugin unload
- [x] 14.5 Re-register MCP proxy tools in ToolRegistry when servers are added/removed

## 15. Plugin Commands (/plugin)

- [x] 15.1 Implement `/plugin list` slash command — display all loaded plugins
- [x] 15.2 Implement `/plugin install <path>` slash command — copy to ~/.claude/plugins/ and load
- [x] 15.3 Implement `/plugin remove <name>` slash command — unload and delete
- [x] 15.4 Register `/plugin list`, `/plugin install`, `/plugin remove` in slash command registry

## 16. Plugin Reload

- [x] 16.1 Implement `/reload-plugins` slash command — unload all, re-discover, reload all
- [x] 16.2 Register `/reload-plugins` in slash command registry
- [x] 16.3 Add test: reload preserves session state (messages, permissions, etc.)

## 17. Startup Integration and Final Verification

- [x] 17.1 Wire plugin loading into CLI startup (after config loading, before session start)
- [x] 17.2 Wire plugin loading into TUI startup
- [x] 17.3 Create a sample test plugin with MCP server, custom agent, and slash command
- [x] 17.4 End-to-end test: sample plugin loads, tools appear, agent works, slash commands dispatch
- [x] 17.5 Verify: `cargo test --workspace` — all tests pass
- [x] 17.6 Verify: `cargo build --workspace` — no warnings
- [x] 17.7 Verify: `cargo clippy --workspace` — no new warnings
