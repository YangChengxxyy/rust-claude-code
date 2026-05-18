## Context

The codebase is now near the end of Phase 4. Iterations 35-40 are implemented, iteration 41 is explicitly out of scope, and iteration 42 has completed the sandbox and SDK public API work. The remaining iteration 43 items are intentionally small but spread across UI, tools, permissions, syntax rendering, and plugin management.

The current architecture already has most of the necessary extension points:

- `AppState` and TUI events carry Todo state and can be extended or reused for Task state.
- WebSearch already uses a backend abstraction enough to support one real provider and domain filtering.
- TUI Markdown rendering already has code block rendering and syntect integration hooks.
- Permission evaluation is centralized in `core::permission::PermissionManager` and surfaced through CLI `--mode` parsing.
- Plugin discovery/listing and manifest concepts already exist, but install/remove need real lifecycle operations instead of usage-only placeholders.

## Goals / Non-Goals

**Goals:**

- Add a TUI Task panel that shows task ID, status, priority, and content, and can be toggled independently from the existing Todo panel workflow.
- Preserve the current Todo panel while adding `Ctrl+T` to switch side panel content between Todo and Task views.
- Add Tavily and SearXNG WebSearch providers behind the existing WebSearch tool contract.
- Keep Brave as the default provider when existing configuration points at Brave or no new provider is configured.
- Ensure TypeScript and TSX code fences receive a distinct syntax mapping rather than falling back to JavaScript or plain text.
- Implement a conservative Auto permission mode: low-risk reads and known-safe Bash commands auto-allow, risky commands and unsafe paths require confirmation, explicit deny still wins.
- Make `/plugin install <path>` and `/plugin remove <name>` actually mutate the local plugin directory and reload plugin state.

**Non-Goals:**

- Replacing Todo with Task everywhere.
- Adding remote plugin marketplace installation.
- Implementing network allow-list enforcement for Auto mode beyond using existing sandbox/network configuration signals.
- Rewriting the Markdown renderer or replacing syntect.
- Adding more WebSearch providers beyond Brave, Tavily, and SearXNG in this cleanup pass.

## Decisions

### Decision: Keep Todo and Task panels as separate side-panel modes

The existing Todo panel is already user-visible and backed by `TodoWriteTool`. Iteration 43 should not remove or rename it. Add a side-panel mode enum in the TUI (`Todo` / `Task`) and use `Ctrl+T` to switch to the Task view. Existing Tab behavior can continue controlling side-panel visibility.

Alternative considered: replace Todo with Task. That aligns with an older iteration 20 spec, but it risks breaking current workflows and is larger than a cleanup change.

### Decision: Task panel consumes AppState task data or existing Task events

If `AppState` already has task storage, render from that state. If Task state currently only flows through events, normalize it into TUI app state when receiving task update events. The panel should not call tools itself; it is a read-only rendering surface.

Alternative considered: query `TaskTool` from the TUI on render. That couples UI rendering to tool execution and introduces async side effects in drawing.

### Decision: WebSearch provider is selected by config/env and hidden from tool input

Keep `WebSearchTool` input focused on search parameters (`query`, domain filters, result limits). Provider choice belongs in config and environment: `webSearch.provider`, `webSearch.apiKey`, `webSearch.baseUrl`, plus `TAVILY_API_KEY` and `SEARXNG_URL` fallbacks.

Alternative considered: add `provider` to each tool call. That exposes infrastructure detail to the model and makes prompt behavior less stable.

### Decision: Implement Tavily and SearXNG as backend structs

Add `TavilySearchBackend` and `SearxngSearchBackend` implementing the existing backend trait. Both must normalize provider-specific responses into the common result model before domain filtering and formatting.

Alternative considered: one generic HTTP backend. Provider response formats differ enough that explicit adapters are clearer and easier to test.

### Decision: Embed syntax definitions only if runtime syntax set lacks TS/TSX

First map common code fence aliases (`ts`, `typescript`, `tsx`, `typescriptreact`) to available syntect syntax names/extensions. If the bundled/default syntax set lacks TypeScript or TSX, embed minimal `.sublime-syntax` resources and load them into the syntax set used by the renderer.

Alternative considered: accept JavaScript highlighting for TypeScript. This fails the cleanup goal because type annotations and TSX-specific constructs remain visibly wrong.

### Decision: Auto mode lives in PermissionManager

`PermissionMode::Auto` should be parsed and stored like other modes, and its core allow/confirm/deny decisions belong in `PermissionManager`. CLI and TUI should not duplicate safety policy.

Alternative considered: handle Auto mode only in CLI before permission checks. That would diverge from SDK/TUI behavior and make tests harder to centralize.

### Decision: Auto mode is conservative and confirmation-first for uncertainty

Auto mode should auto-allow read-only tools and a small allowlist of safe shell commands (`git status`, `git diff`, `ls`, `pwd`, `cat`, `rg`, similar read-only inspection). Dangerous Bash patterns, network commands, writes outside safe paths, and unknown write operations should require confirmation rather than being denied by default unless an explicit deny rule matches.

Alternative considered: aggressively allow all sandboxed commands. Even with sandboxing, command intent can still be destructive inside allowed paths, so dangerous patterns should still escalate.

### Decision: Plugin install copies validated local directories

`/plugin install <path>` should validate `plugin.json`, use manifest `name` and `version`, copy the directory into `~/.claude/plugins/<name>/`, refuse invalid manifests, and reload plugin state after install. If the target exists, fail with a clear message unless an existing overwrite flag already exists in the command system.

Alternative considered: symlink installs. Symlinks are useful for development but create lifecycle ambiguity for remove; copying is safer for the first complete implementation.

### Decision: Plugin remove only removes user-installed plugins

`/plugin remove <name>` should unload plugin resources and delete `~/.claude/plugins/<name>/`. It should not delete project-level `.claude/plugins/<name>/` because those are repository-managed files.

Alternative considered: remove whichever plugin is active after precedence resolution. That could accidentally delete project files and violates user expectations.

## Risks / Trade-offs

- [Risk] Task state may be represented differently from Todo state. Mitigation: keep the Task panel adapter small and normalize only the fields required for rendering.
- [Risk] WebSearch provider config can become fragmented across env/settings/config. Mitigation: document one resolution order and test provider selection explicitly.
- [Risk] SearXNG instances vary in enabled formats and rate limiting. Mitigation: require JSON endpoint support and return clear backend errors.
- [Risk] TypeScript syntax resources can increase binary size. Mitigation: embed only if existing syntax support is insufficient.
- [Risk] Auto mode may surprise users if it allows too much. Mitigation: keep the initial allowlist narrow and route uncertain cases to existing confirmation UI.
- [Risk] Plugin install/remove can conflict with already loaded MCP/custom-agent resources. Mitigation: perform lifecycle unload/reload through the existing plugin manager instead of only modifying files.

## Migration Plan

1. Add Auto mode parsing and permission tests first, because it touches core behavior but can be isolated.
2. Add WebSearch provider config and backend adapters with mocked HTTP/provider tests.
3. Add TUI Task side-panel state/rendering and keybinding tests.
4. Add TS/TSX syntax alias/resource support and renderer tests.
5. Add plugin install/remove lifecycle methods and slash command integration tests.
6. Run formatting and targeted crate tests, then the workspace test suite.

Rollback is straightforward for each item because they are additive: remove the new mode, provider adapters, panel mode, syntax resources, or plugin lifecycle methods independently. Existing default behavior remains unchanged when new configuration or commands are not used.

## Open Questions

- Should `/plugin install` later support a development-mode symlink flag? Not required for iteration 43.
- Should Auto mode eventually inspect command output before allowing dependent actions? The spec records the intent, but the cleanup implementation should start with pre-execution safety checks and existing confirmation fallback.
