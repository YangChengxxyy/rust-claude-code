## 1. Auto Permission Mode

- [x] 1.1 Add `PermissionMode::Auto` parsing, serialization, display text, config loading support, and `--mode auto` CLI selection.
- [x] 1.2 Extend `PermissionManager` so explicit deny rules still win, read-only tools are auto-approved, and unknown/risky operations return confirmation-required behavior.
- [x] 1.3 Add conservative Bash safety classification for known read-only commands, dangerous command patterns, network commands, and writes outside safe path scope.
- [x] 1.4 Add tests for Auto mode parsing, explicit-deny precedence, read-only auto-allow, safe Bash auto-allow, dangerous Bash confirmation, and unsafe file path confirmation.

## 2. WebSearch Multi-Backend Support

- [x] 2.1 Add WebSearch provider configuration for `brave`, `tavily`, and `searxng`, including environment fallbacks for `TAVILY_API_KEY` and `SEARXNG_URL`.
- [x] 2.2 Implement `TavilySearchBackend` and normalize Tavily responses into the existing search result model.
- [x] 2.3 Implement `SearxngSearchBackend` using a JSON-capable SearXNG instance and normalize responses into the existing search result model.
- [x] 2.4 Preserve existing Brave/default behavior and apply domain filters after provider normalization.
- [x] 2.5 Add mocked backend tests for provider selection, Tavily success/failure, SearXNG success/failure, no results, and domain filtering.

## 3. TUI Task Panel

- [x] 3.1 Add TUI side-panel mode state for Todo versus Task while preserving the existing Todo panel and Tab visibility behavior.
- [x] 3.2 Add `Ctrl+T` key handling to switch the side panel to Task view and back to Todo view.
- [x] 3.3 Add Task panel rendering with task ID, status, priority, and short description, including an empty-state placeholder.
- [x] 3.4 Wire Task update events or AppState task snapshots into TUI app state so task changes refresh on the next render.
- [x] 3.5 Add TUI tests for panel mode switching, Task rendering, empty state, and update propagation.

## 4. TypeScript And TSX Syntax Highlighting

- [x] 4.1 Map TypeScript code fence aliases (`ts`, `typescript`) and TSX aliases (`tsx`, `typescriptreact`) to dedicated syntax lookup paths.
- [x] 4.2 Detect whether the existing syntect syntax set supports TypeScript/TSX and embed minimal `.sublime-syntax` resources if needed.
- [x] 4.3 Add renderer tests proving TypeScript type annotations and TSX markup do not fall back to plain text.

## 5. Plugin Install And Remove

- [x] 5.1 Add `PluginManager::install_from_path` to validate local plugin manifests and copy plugin directories to `~/.claude/plugins/<name>/`.
- [x] 5.2 Add `PluginManager::remove_installed` to unload and remove user-installed plugins without deleting project-level plugin directories.
- [x] 5.3 Wire `/plugin install <path>` and `/plugin remove <name>` slash commands to the real lifecycle methods and reload plugin state after successful changes.
- [x] 5.4 Add plugin tests for valid install, missing path, missing manifest, invalid version/name, existing target conflict, remove installed plugin, and remove nonexistent plugin.

## 6. Verification

- [x] 6.1 Run `cargo fmt --all`.
- [x] 6.2 Run targeted tests for permission, WebSearch, TUI, syntax highlighting, and plugin changes.
- [x] 6.3 Run `cargo test --workspace`.
