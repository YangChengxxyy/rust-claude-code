## Context

The Rust Claude Code currently has 5 tools: Bash, FileRead, FileEdit, FileWrite, TodoWrite. Code exploration relies on `BashTool` running shell commands like `find` or `grep`, which lack structured output and fine-grained permission control. The original Claude Code has dedicated `Glob` and `Grep` tools that are among the most frequently invoked during coding sessions.

The `tools` crate follows a consistent pattern: each tool is a struct implementing the `Tool` trait (`info()`, `is_read_only()`, `is_concurrency_safe()`, `execute()`), registered by name in `ToolRegistry`. Both new tools are read-only and concurrency-safe, fitting cleanly into the existing concurrency model where safe tools run in parallel via `join_all`.

## Goals / Non-Goals

**Goals:**
- Implement `GlobTool` for fast file pattern matching with glob syntax
- Implement `GrepTool` for regex-based content search with filtering and context
- Both tools follow existing `Tool` trait patterns and integrate with ToolRegistry, QueryLoop, permissions, and system prompt
- Support the most common parameters from the original Claude Code implementation

**Non-Goals:**
- Full ripgrep binary integration (we use Rust regex/glob crates directly)
- `.gitignore`-aware filtering in v1 (can be added later with the `ignore` crate)
- Multiline regex support in v1
- Incremental/streaming search results
- File content caching or indexing

## Decisions

### 1. Glob implementation: `glob` crate

Use the `glob` crate for pattern matching. It's the most widely-used Rust glob library, supports standard glob syntax (`*`, `**`, `?`, `[...]`), and handles recursive directory traversal efficiently.

**Alternative considered**: `walkdir` + manual pattern matching — more control but reimplements what `glob` already provides.

### 2. Grep implementation: `grep-regex` + `grep-searcher` from the ripgrep ecosystem

Use the `grep-regex` and `grep-searcher` crates from BurntSushi's ripgrep project. These provide the same search engine as `rg` but as a library, giving us structured results without spawning a subprocess.

**Alternative considered**: 
- Spawning `rg` as a subprocess — fragile (requires `rg` installed), harder to parse output, less portable.
- Using `regex` crate with manual file walking — works but reimplements line-oriented search, context handling, and file type filtering that `grep-searcher` already solves.

**Decision**: Start with the simpler `regex` + `walkdir` approach for v1. The `grep-searcher`/`grep-regex` crates add complexity and their APIs are designed for the ripgrep binary's specific needs. We can always upgrade to them later if performance becomes a bottleneck. For v1, `walkdir` for directory traversal + `regex` for matching + manual context line handling is straightforward, testable, and sufficient.

### 3. GrepTool output modes

Support two output modes matching the original Claude Code:
- `files_with_matches` (default): return only file paths containing matches
- `content`: return matching lines with optional context (`-A`, `-B`, `-C` lines)

A `count` mode can be added later.

### 4. Result limiting

Both tools enforce a `head_limit` parameter (default 250 for GrepTool) to prevent enormous result sets from blowing up context. GlobTool returns results sorted by modification time (newest first) to surface the most relevant files.

### 5. Tool naming

Use `Glob` and `Grep` as the tool names in the registry, matching the original Claude Code's naming convention (`GlobTool` class → `Glob` registered name).

### 6. Permission classification

Both tools are `is_read_only() = true` and `is_concurrency_safe() = true`. This means:
- They are auto-allowed in all permission modes (read-only tools pass all modes except explicit deny rules)
- They run in parallel with other concurrency-safe tools (currently only `FileRead`)

## Risks / Trade-offs

**[Performance on large repos]** → Glob/grep on very large repositories (100k+ files) could be slow without `.gitignore` filtering. Mitigation: enforce `head_limit` defaults, document that `.gitignore`-aware filtering is planned for a future iteration.

**[Regex complexity]** → Users (the LLM) could submit expensive regex patterns. Mitigation: use `regex` crate which has built-in protection against catastrophic backtracking (it uses finite automata, not backtracking).

**[Missing `rg` feature parity]** → The `--type` filter in original Claude Code maps to ripgrep's type definitions. Mitigation: implement a basic type→extension mapping for common types (`js`, `py`, `rs`, `go`, `ts`, etc.) in v1.
