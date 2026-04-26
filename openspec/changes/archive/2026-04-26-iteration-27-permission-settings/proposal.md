## Why

The permission system only supports tool-name + command-prefix matching, so there is no way to express path-scoped rules like "allow edits to `src/**/*.ts`" or "deny reads of `.env`". Settings lack a local-override layer (`.claude/settings.local.json`) and per-path rule files (`.claude/rules/*.md`), making personal project config and path-scoped instructions impossible. And there is no interactive way to view or manage permission rules at runtime. These gaps limit both the expressiveness of the security model and day-to-day configuration ergonomics.

## What Changes

- Extend `PermissionRule` to support path-glob patterns for file-oriented tools (`FileEdit`, `FileWrite`, `FileRead`, `Bash`).
- Add a new path-pattern syntax supporting project-relative (`/path`), CWD-relative (`./path`), home-relative (`~/path`), and absolute (`//path`) prefixes, with glob wildcards (`*`, `**`).
- Introduce a three-level rule evaluation: Deny > Ask > Allow (adding `Ask` as a new rule type that explicitly forces a confirmation prompt).
- Auto-extract file paths from `FileEdit`, `FileWrite`, and `FileRead` tool inputs to participate in path-based rule matching.
- Support `.claude/settings.local.json` as a per-user local project settings layer (gitignored, higher priority than `.claude/settings.json`).
- Support `CLAUDE.local.md` as a per-user local project instructions file (gitignored).
- Support `.claude/rules/*.md` path-scoped rule files with `paths` frontmatter to inject instructions only when working in matching directories.
- Update settings merge priority to: CLI > env > local project > shared project > user > default.
- Add `/permissions` slash command to view all rules with source annotations, and add/delete rules interactively.
- Add `/init` slash command to scaffold a `.claude/` directory with a basic `CLAUDE.md`.
- Add `/status` slash command to show a consolidated overview of model, permissions, MCP, hooks, and memory state.

## Capabilities

### New Capabilities

- `permission-path-wildcards`: Path-glob matching for permission rules, three-level evaluation (Deny/Ask/Allow), and file-path extraction from tool inputs.
- `settings-local-layer`: `.claude/settings.local.json` local override, `CLAUDE.local.md` personal instructions, `.claude/rules/*.md` path-scoped rule files, and updated merge priority chain.
- `slash-status-commands`: `/permissions`, `/init`, and `/status` slash commands for runtime inspection and project scaffolding.

### Modified Capabilities

- `settings-merge`: Merge priority chain updated to include local project layer; settings discovery extended to find `.claude/settings.local.json`.
- `claude-md-discovery`: Discovery extended to find `CLAUDE.local.md` files and `.claude/rules/*.md` path-scoped files.
- `claude-md-injection`: Injection updated to include `CLAUDE.local.md` content and path-matched rule files in the system prompt.

## Impact

- `crates/core`: `PermissionRule` gains path-pattern fields and `Ask` rule type; `PermissionManager` evaluates three-level rules with path extraction. `ClaudeSettings` discovery adds local layer. `claude_md` discovery adds `CLAUDE.local.md` and `.claude/rules/*.md`.
- `crates/tools`: File-oriented tools may need to expose their target path for permission matching.
- `crates/cli`: `system_prompt` injection updated for new CLAUDE.md sources. Config resolution updated for local settings layer.
- `crates/tui`: Three new slash commands (`/permissions`, `/init`, `/status`) with corresponding `UserCommand` variants and handlers.
- Project convention: `.claude/settings.local.json` and `CLAUDE.local.md` should be added to `.gitignore` templates.
