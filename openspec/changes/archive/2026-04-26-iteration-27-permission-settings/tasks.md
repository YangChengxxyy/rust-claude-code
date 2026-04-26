## 1. Permission Path Wildcards

- [x] 1.1 Add `glob-match` dependency to `crates/core/Cargo.toml`
- [x] 1.2 Extend `RuleType` with `Ask` variant; update serialization, display, and parsing
- [x] 1.3 Add `path_pattern: Option<String>` field to `PermissionRule`; update compact string parsing to handle `Tool(cmd, path)` and `Tool(, path)` syntax
- [x] 1.4 Add `PermissionRule` serialization round-trip for path patterns (compact string format)
- [x] 1.5 Implement path-pattern prefix resolution (`/`, `./`, `~/`, `//`) against project root, session CWD, and home dir
- [x] 1.6 Implement glob matching of resolved paths against path patterns using `glob-match`
- [x] 1.7 Add `extract_file_path(tool_name, input)` helper for `FileEdit`, `FileWrite`, `FileRead` path extraction
- [x] 1.8 Update `PermissionManager::check()` to evaluate three-level Deny > Ask > Allow, incorporating path matching when rules have path patterns
- [x] 1.9 Add tests: path-only rules, combined command+path rules, prefix resolution, Ask tier precedence, BypassPermissions override, Plan mode override
- [x] 1.10 Run `cargo test -p rust-claude-core` and verify all permission tests pass

## 2. Settings Local Layer

- [x] 2.1 Extend `ClaudeSettings::load_project()` to discover and load `.claude/settings.local.json` alongside `.claude/settings.json`
- [x] 2.2 Update merge chain to apply local-project layer between shared-project and user layers
- [x] 2.3 Add tests: local-only, shared-only, both present with local overriding, permissions concatenation across three layers
- [x] 2.4 Run `cargo test -p rust-claude-core` and verify all settings tests pass

## 3. CLAUDE.local.md and Rules Discovery

- [x] 3.1 Extend `discover_claude_md()` to find `CLAUDE.local.md` in each discovery directory (global + project), inserting after corresponding `CLAUDE.md`
- [x] 3.2 Add optional `source_type` field to `ClaudeMdFile` to distinguish global, project, local, and rule sources
- [x] 3.3 Implement `.claude/rules/*.md` discovery in project root's `.claude/rules/` directory
- [x] 3.4 Implement YAML frontmatter parsing for `paths` field in rule files (simple `---` delimiter scanner + `serde_yaml`)
- [x] 3.5 Add `serde_yaml` dependency to `crates/core/Cargo.toml` if not already present
- [x] 3.6 Add tests: CLAUDE.local.md next to CLAUDE.md ordering, CLAUDE.local.md without CLAUDE.md, global CLAUDE.local.md, rules discovery, frontmatter parsing, no-rules-dir graceful handling
- [x] 3.7 Run `cargo test -p rust-claude-core` and verify all claude_md tests pass

## 4. CLAUDE.md Injection Updates

- [x] 4.1 Update `build_claude_md_section()` to annotate CLAUDE.local.md files with local-specific source headers
- [x] 4.2 Add CWD-based path matching for rule files: evaluate `paths` globs against session CWD relative to project root
- [x] 4.3 Inject matched rule files after all CLAUDE.md/CLAUDE.local.md entries with rule-specific annotations
- [x] 4.4 Update truncation logic: truncate global/root-most first, then rule files, then preserve leaf-most CLAUDE.md
- [x] 4.5 Add tests: local file annotation, rule file CWD matching and exclusion, rule file injection position, truncation with mixed file types
- [x] 4.6 Run `cargo test -p rust-claude-cli` and verify system prompt tests pass

## 5. Slash Commands

- [x] 5.1 Add `UserCommand` variants: `ShowPermissions`, `InitProject`, `ShowStatus`
- [x] 5.2 Add `/permissions`, `/init`, `/status` to `SLASH_COMMANDS` spec list with usage and description
- [x] 5.3 Implement `/permissions` handler: gather rules from PermissionManager, group by source, format and display via `AppEvent::SystemMessage`
- [x] 5.4 Implement `/init` handler: find git root (or CWD), create `.claude/` dir and starter `CLAUDE.md` if absent, display result
- [x] 5.5 Implement `/status` handler: collect model (with source), permission mode, rule count, MCP server count/status, hooks count, memory count; format and display
- [x] 5.6 Add `handle_slash_command` match arms for the three new commands
- [x] 5.7 Add tests for slash command parsing and basic handler behavior
- [x] 5.8 Run `cargo test -p rust-claude-tui` and verify all TUI tests pass

## 6. Integration and Verification

- [x] 6.1 Run `cargo fmt --all`
- [x] 6.2 Run `cargo test -p rust-claude-core`
- [x] 6.3 Run `cargo test -p rust-claude-cli`
- [x] 6.4 Run `cargo test -p rust-claude-tui`
- [x] 6.5 Run `cargo test --workspace`
- [x] 6.6 Manual test: add a path-glob permission rule in config, invoke FileEdit on matching/non-matching paths, verify correct allow/deny behavior
- [x] 6.7 Manual test: create `.claude/settings.local.json` with a model override, verify `/config` shows the local source
- [x] 6.8 Manual test: create `CLAUDE.local.md` and `.claude/rules/test.md`, verify `/context` shows both in the context breakdown
- [x] 6.9 Manual test: run `/permissions`, `/init`, `/status` and verify correct output
