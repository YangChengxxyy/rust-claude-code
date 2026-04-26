## Context

The permission system (`PermissionRule` in `crates/core/src/permission.rs`) currently supports tool-name matching with an optional command-prefix pattern (e.g., `Bash(git *)`). There is no way to express file-path conditions, so rules like "allow edits under `src/`" or "deny reads of `.env`" cannot be represented. The rule evaluation is two-level: Deny > Allow, with no explicit "ask for confirmation" tier.

Settings loading (`crates/core/src/settings.rs`) discovers `~/.claude/settings.json` (user) and `.claude/settings.json` (project) and merges them. There is no `.claude/settings.local.json` layer for personal per-project overrides, no `CLAUDE.local.md` for personal instructions, and no `.claude/rules/*.md` for path-scoped instruction files.

CLAUDE.md discovery (`crates/core/src/claude_md.rs`) finds global and project `CLAUDE.md` files by walking up from CWD to git root. It does not look for `CLAUDE.local.md` or `.claude/rules/*.md`.

Slash commands are dispatched in `crates/tui/src/app.rs` via a static spec list and a `match` block. There is no `/permissions`, `/init`, or `/status` command.

## Goals / Non-Goals

**Goals:**

- Add path-glob matching to permission rules so file-oriented tools can be governed by path patterns.
- Introduce a three-level rule evaluation (Deny > Ask > Allow) with `Ask` as an explicit confirmation tier.
- Auto-extract file paths from `FileEdit`, `FileWrite`, `FileRead` tool inputs for rule matching.
- Support `.claude/settings.local.json` as a gitignored local-project layer with higher priority than `.claude/settings.json`.
- Support `CLAUDE.local.md` alongside `CLAUDE.md` in discovery, injected after its corresponding `CLAUDE.md`.
- Support `.claude/rules/*.md` files with `paths` YAML frontmatter for path-scoped instruction injection.
- Add `/permissions`, `/init`, `/status` slash commands.
- Keep all new logic testable without requiring a live terminal or API.

**Non-Goals:**

- Regex-based permission patterns. Glob patterns (`*`, `**`) are sufficient for this iteration.
- Interactive permission confirmation UI in the query loop. `NeedsConfirmation` is still auto-denied; this iteration adds the rule expressiveness, not the TUI confirmation dialog.
- Full settings schema validation or migration tooling.
- Rewriting the slash command dispatch into a unified registry. That is desirable but out of scope for this iteration.

## Decisions

### D1: Path-pattern syntax in PermissionRule

**Choice**: Extend `PermissionRule` with an optional `path_pattern: Option<String>` field. The compact string syntax becomes `Tool(command_pattern, /path/pattern)` for rules that have both, or `Tool(, /path/pattern)` for path-only rules, or the existing `Tool(command_pattern)` for command-only rules.

Path pattern prefix semantics:
- `/path` — relative to project root (git root)
- `./path` — relative to session CWD
- `~/path` — relative to user home directory
- `//path` — absolute filesystem path

Glob matching uses the `glob-match` crate (already common in Rust ecosystem, zero-alloc). Patterns support `*` (single segment) and `**` (recursive).

**Alternatives considered**:
- Encode path into the existing `pattern` field. This conflates command patterns and path patterns, making parsing ambiguous for tools that have both (e.g., `Bash` can have a command prefix AND produce file paths).
- Use regex. Overly powerful and harder for users to write correctly.

**Rationale**: Separate fields keep the data model clean. The prefix syntax mirrors common conventions and maps naturally to path resolution.

### D2: Three-level rule evaluation with Ask tier

**Choice**: Add `Ask` to `RuleType` (alongside `Allow` and `Deny`). Evaluation order becomes: Deny rules > Ask rules > Allow rules > mode-specific default. When an `Ask` rule matches, the result is `NeedsConfirmation` regardless of the mode (except `BypassPermissions` which still allows everything, and `Plan` which still denies all writes).

**Alternatives considered**:
- Keep two tiers and rely on mode defaults for confirmation. This prevents users from saying "ask me about edits to production config files" while auto-allowing other edits.

**Rationale**: The Ask tier gives users precise control over which operations prompt confirmation, independent of the global mode.

### D3: File-path extraction for rule matching

**Choice**: Add a `fn extract_file_path(tool_name: &str, input: &serde_json::Value) -> Option<String>` helper in the permission module. For `FileEdit` and `FileWrite`, extract from `file_path` field. For `FileRead`, extract from `file_path` field. For `Bash`, do not extract paths (Bash commands are too varied; command-prefix matching remains the primary mechanism).

The extracted path is resolved against the session CWD before matching against path patterns.

**Alternatives considered**:
- Have each tool implement a `target_paths()` method on the `Tool` trait. This is cleaner architecturally but requires changes to the trait and all implementations. Defer to a future iteration.

**Rationale**: A simple extraction function in the permission module keeps the change localized and avoids trait changes.

### D4: Settings local layer discovery

**Choice**: In `ClaudeSettings::load_project()`, after finding `.claude/settings.json`, also look for `.claude/settings.local.json` in the same directory. The local layer has higher priority and is merged on top of the shared project layer before merging with the user layer.

Merge priority becomes: CLI > env > project-local > project-shared > user > default.

**Alternatives considered**:
- A separate discovery function for local settings. This duplicates the directory-walk logic.

**Rationale**: Reusing the existing discovery path and adding one more file lookup keeps the code simple. The merge function already handles layered precedence.

### D5: CLAUDE.local.md and .claude/rules/*.md discovery

**Choice**: Extend `discover_claude_md()` to also find:
1. `CLAUDE.local.md` — discovered in the same directories as `CLAUDE.md`, inserted immediately after its corresponding `CLAUDE.md` in the ordered list.
2. `.claude/rules/*.md` — discovered in the project root's `.claude/rules/` directory. Each file may have YAML frontmatter with a `paths` field (array of glob patterns). Files are only included when the session CWD matches at least one pattern. Files without `paths` frontmatter are always included.

`ClaudeMdFile` gains an optional `scope` field to distinguish global, project, local, and rule sources for display in `/context`.

**Alternatives considered**:
- Discover rules files in every ancestor directory. This is complex and unlikely to be useful; `.claude/rules/` at the project root is sufficient.

**Rationale**: Local files next to their public counterparts is intuitive. Path-scoped rules in a dedicated directory keeps them organized and discoverable.

### D6: /permissions, /init, /status commands

**Choice**: Add three new slash commands following the existing dispatch pattern:

- `/permissions` — Displays all active rules grouped by source (built-in, user config, project settings, session). No interactive add/delete in this iteration (that requires a more complex TUI modal); instead, show the rules and tell the user how to edit the config files.
- `/init` — Creates `.claude/` directory and a starter `CLAUDE.md` in the project root (git root). Warns if already exists.
- `/status` — Shows a consolidated view: model name, permission mode, active rules count, MCP servers count + status, hooks count, memory entries count.

Each adds a `UserCommand` variant and a handler in the CLI event loop.

**Alternatives considered**:
- Interactive `/permissions add/remove` with TUI modal. Deferred — showing rules with source annotations is the high-value part; editing can use config files for now.

**Rationale**: Read-only commands are simpler to implement and already useful. Interactive editing can be added in a later iteration.

## Risks / Trade-offs

- **[Path resolution depends on CWD]** -> `./path` patterns resolve against session CWD which may change during a session. Document that `./` is evaluated at rule-check time, not rule-creation time. For stable paths, use `/` (project-relative) instead.
- **[Glob matching performance]** -> Rules are checked per tool call and the rule list is typically small (<50 rules). Glob compilation could be cached if this becomes measurable, but is unlikely to matter.
- **[YAML frontmatter parsing in rules files]** -> Need a lightweight parser for the `paths` frontmatter. Use a simple `---` delimiter scanner and `serde_yaml` for just the frontmatter block, avoiding a full markdown AST dependency.
- **[settings.local.json merge complexity]** -> Adding a third settings layer (user, project-shared, project-local) increases merge permutations. The existing `ClaudeSettings::merge()` is already pair-wise; applying it twice (shared+local, then result+user) keeps the logic linear.
- **[/init could overwrite user content]** -> Check for existing `.claude/CLAUDE.md` and refuse to overwrite. Only create if absent.
