## Context

The `crates/cli/` crate currently contains both the CLI binary (`main.rs`) AND all core agent logic: `QueryLoop` (2003 lines), `HookRunner` (973 lines), `CompactionService` (777 lines), and the system prompt builder (579 lines). The `QueryLoop` is tightly coupled to `rust_claude_tui::TuiBridge` through ~50 direct field accesses for streaming events, permission dialogs, and user questions. This makes it impossible to use the agent loop outside the CLI/TUI context.

After 33 iterations, all major features are implemented: 17 built-in tools, MCP (stdio/sse/http), hooks (7 event types), custom agents, auto-memory, sandbox, auto-permission-mode, stream controls, syntax highlighting, diff preview, and 32 slash commands. The final architectural gap is extracting the agent runtime into a reusable library and adding an extension system.

## Goals / Non-Goals

**Goals:**

1. Create `crates/sdk/` crate that can be compiled, tested, and used without `tui/` or `cli/` dependencies
2. Decouple `QueryLoop` from `TuiBridge` using abstract traits (`OutputSink`, `PermissionUI`, `UserQuestionUI`)
3. Provide `SessionBuilder` API: ergonomic construction and headless agent execution
4. Define `PluginManifest` format and implement plugin discovery, loading, and lifecycle management
5. Refactor slash commands from static `match` dispatch to dynamic trait-based registry for plugin extensibility
6. All 738 existing tests continue to pass with no behavior regressions

**Non-Goals:**

- Native code plugin execution (wasm, shared libraries) — configuration-only plugins for this iteration
- Plugin marketplace or remote registry — local file-system plugins only
- Plugin-provided `Tool` implementations via code — tools are provided through MCP servers in plugin manifests
- Changing the CLI binary interface or TUI behavior beyond what's needed for the refactor
- Agent Teams, remote sessions, or IDE integration

## Decisions

### Decision 1: Trait-based abstraction over TuiBridge

**Chosen**: Three separate traits (`OutputSink`, `PermissionUI`, `UserQuestionUI`) replacing `Option<TuiBridge>`.

**Rationale**: Separation of concerns. A headless SDK user might want output streaming without interactive permissions. Splitting into three traits lets each be independently provided or omitted. The `TuiBridge` implements all three, keeping existing behavior intact.

**Alternatives considered**:
- Single `AgentUI` trait: simpler but forces all-or-nothing implementation
- Async callback closures (like `AgentContextRunSubagent`): works for simple cases but harder to document and type-check
- Keep `TuiBridge` and add a `NoopBridge`: defeats the purpose of zero `tui/` dependency

### Decision 2: Slash command registration model

**Chosen**: Dynamic trait-based (`SlashCommand` trait + `SlashCommandRegistry` with `register()`) in `tui/`.

**Rationale**: Plugins need to register commands at runtime. A trait with `name()`, `usage()`, `description()`, and `execute()` mirrors the existing `SlashCommandSpec` struct but adds behavior. The existing 32 commands become trait implementations, keeping identical behavior while enabling dynamic registration.

**Alternatives considered**:
- Extend `UserCommand` enum with `PluginCommand { name, args }`: minimal code change but pushes all command logic to the worker, making the TUI a dumb terminal
- String-based template expansion (commands are just prompt templates): simplest but zero interactivity — can't implement `/plugin list`, `/reload-plugins`, etc.
- Keep static dispatch and only allow plugin commands through a separate code path: two dispatch systems, maintenance burden

### Decision 3: Plugin code execution model

**Chosen**: Configuration-only (`plugin.json` manifest declaring MCP servers, custom agents, and slash command templates).

**Rationale**: All three extension mechanisms already exist in the codebase. MCP servers provide tools via stdio/SSE/HTTP. Custom agents are YAML frontmatter definitions. Slash command templates map user input to agent prompts. A plugin manifest ties these together into a named, versioned bundle without requiring any code execution.

**Alternatives considered**:
- `libloading` (native shared libraries): full power but unsafe, platform-dependent, no sandboxing
- `wasmtime` (WebAssembly): safe sandboxing but heavy dependency, limited filesystem/network access
- Shell scripts: simple but inconsistent cross-platform behavior and security concerns

### Decision 4: Crate structure for SDK

**Chosen**: New `crates/sdk/` crate at the same level as existing crates.

```
crates/sdk/
├── Cargo.toml
└── src/
    ├── lib.rs              — Public re-exports
    ├── output.rs            — OutputSink, PermissionUI, UserQuestionUI traits
    ├── agent_loop.rs        — QueryLoop (moved from cli/, refactored)
    ├── hooks.rs             — HookRunner (moved from cli/)
    ├── compaction.rs        — CompactionService (moved from cli/)
    ├── system_prompt.rs     — System prompt builder (moved from cli/)
    ├── session.rs           — SessionBuilder + Session<C>
    └── plugin.rs            — PluginManifest, PluginLoader (34b)
```

**Rationale**: The SDK is the "library layer" between `api`/`core`/`tools` and `cli`/`tui`. It depends on `api`, `core`, `tools`, `mcp` (for plugin MCP server loading). It does NOT depend on `tui` or `cli`. The `cli` crate depends on `sdk` and becomes a thin adapter.

### Decision 5: Session API shape

**Chosen**:
```rust
pub struct Session<C: ModelClient> { ... }

impl<C: ModelClient> Session<C> {
    pub fn builder() -> SessionBuilder<C>;
    pub async fn send(&mut self, prompt: String) -> Result<Message, Error>;
    pub async fn send_streaming(&mut self, prompt: String) -> Result<EventStream, Error>;
}
```

`Session` owns `AppState` internally. The `SessionBuilder` accepts: `client`, `tools` (via `ToolRegistry`), `output` (`Option<Box<dyn OutputSink>>`), `permission_ui`, `user_question_ui`, `max_rounds`, `compaction_config`, `hook_runner`, `agent_config`. Building a `Session` without a `client` or `tools` is a compile error (builder requires them).

### Decision 6: Plugin discovery paths

**Chosen**: Two-tier discovery: `~/.claude/plugins/` (user-global) and `.claude/plugins/` (project-local). Each directory scanned for subdirectories containing `plugin.json`. Project plugins override user plugins with the same name.

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| ~50 `self.bridge` calls in QueryLoop need individual refactoring — high chance of missing one | One method at a time; compiler catches type mismatches; existing tests verify behavior |
| 32 slash commands need refactoring from `match` arms to `SlashCommand` impls — large but mechanical | Extract each command body to a standalone function first, then wrap in trait impls — no logic changes |
| `McpManager` currently has no runtime `add_server()` / `remove_server()` — needed for plugin hot-reload | Add these methods in 34b; McpManager holds `HashMap<String, Mutex<ConnectedServer>>`, dynamic add/remove is structurally possible |
| Recursive `AgentContext` builder uses `Arc<Mutex<Option<...>>>` self-reference — fragile pattern | Encapsulate in `SessionBuilder` as internal detail; SDK users never see this |
| Moving `system_prompt.rs` may expose the massive `CORE_PROMPT` static string publicly | Keep `CORE_PROMPT` as `pub(crate)` in sdk; only expose the builder function |
| `CompactionService` depends on `ModelClient` from `api/` — sdk/ must depend on api/ | Acceptable; sdk/ already depends on api/ for QueryLoop |
| Plugin slash commands with the same name as built-in commands | Plugin registry checks built-in registry first; duplicate names are rejected at load time with error |

## Migration Plan

1. **34a**: Create `sdk/` crate, move code, define traits, refactor QueryLoop, adapt `tui/` and `cli/`
   - Rollback: revert to previous commit — all changes are additive or internal refactors
   - Validation: `cargo test --workspace` must pass at every commit
2. **34b**: Add plugin manifest, discovery, loading, slash command refactoring, plugin commands
   - Rollback: revert to 34a state
   - Validation: load a sample plugin and verify its MCP server tools, custom agents, and slash commands appear

## Open Questions

- Should `HookRunner` stay in `sdk/` or move to `core/`? Leaning toward `sdk/` since it uses `tokio::process` and `serde_json` for execution — runtime concerns, not pure types.
- Should the SDK provide an `EventStream` type or reuse `futures::Stream<Item = SessionEvent>`? Leaning toward a concrete enum `SessionEvent` with `impl Stream` for better documentation.
- For 34b: should plugin slash commands support interactive TUI dialogs (e.g., `/plugin install` shows a confirmation), or only fire-and-forget prompt submission? Initial scope: fire-and-forget, with interactive dialogs as a future enhancement.
