## Why

Iteration 43 is the final Phase 4 cleanup pass. The main resilience, performance, sandbox, and SDK work is already complete, but several Phase 2-3 leftovers still affect day-to-day completeness: the TUI only has a Todo side panel, WebSearch is effectively tied to the Brave backend, TypeScript/TSX highlighting is not first-class, Auto permission mode is not implemented, and plugin install/remove commands are still incomplete in the current runtime.

This change closes those remaining gaps so the fourth phase can be considered complete without carrying forward small but visible usability holes.

## What Changes

- Add a TUI Task side panel that can display task state separately from the existing Todo panel, with `Ctrl+T` switching panel content.
- Add WebSearch provider selection for Brave, Tavily, and SearXNG through config/environment without changing the tool input schema.
- Improve TypeScript and TSX syntax highlighting by ensuring syntect can resolve appropriate syntax definitions.
- Add `PermissionMode::Auto`, `--mode auto`, and baseline safety checks that auto-allow low-risk operations while escalating risky ones to confirmation.
- Complete local plugin lifecycle commands for `/plugin install <path>` and `/plugin remove <name>`, including manifest validation, copy/remove behavior, and reload after changes.

## Capabilities

### New Capabilities

- `tui-task-panel`: TUI can show task IDs, status, priority, and descriptions in a side panel with realtime updates.

### Modified Capabilities

- `web-search`: WebSearch supports configurable Brave, Tavily, and SearXNG backends.
- `tui-syntax-highlighting`: Code block highlighting includes explicit TypeScript and TSX support.
- `auto-permission-mode`: Auto mode is selectable and applies conservative safety checks for read, file, and Bash operations.
- `plugin-system`: Plugin install/remove commands perform real local filesystem lifecycle operations and reload plugin state.

## Impact

- Affected crates: `core` for permission mode/config additions, `tools` for WebSearch backend selection and plugin lifecycle helpers if currently located there, `tui` for task panel and syntax highlighting, and `cli` for slash command and permission-mode wiring.
- User-visible behavior: `--mode auto`, `Ctrl+T`, Tavily/SearXNG WebSearch configuration, better TS/TSX rendering, and functional `/plugin install` / `/plugin remove`.
- Compatibility: existing permission modes, Brave WebSearch behavior, Todo panel behavior, and plugin list behavior remain unchanged unless the new options are used.
- Dependencies: may require embedding TypeScript/TSX `.sublime-syntax` resources if the existing syntect syntax set does not include them.
