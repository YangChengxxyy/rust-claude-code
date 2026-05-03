## Why

Running rust-claude-code in an untrusted directory allows malicious `.claude/settings.json` files to execute arbitrary commands via `apiKeyHelper` without any user confirmation. Additionally, `FileEditTool` and `FileWriteTool` have no mechanism to detect whether a file was externally modified since the last read, risking silent overwrites of concurrent user edits. These are security and data-integrity gaps that must be closed before other feature work.

## What Changes

- Introduce a **Trust Manager** that checks whether the current project directory is trusted before loading `apiKeyHelper` or `settings.json` env variables
- Add a **TUI trust confirmation dialog** shown on first run in an untrusted directory
- Add `--trust` CLI flag to skip the dialog (for CI/automation)
- In `--print` mode, refuse to run in untrusted directories (prompt user to use `--trust`)
- Introduce a **File State Cache** (LRU, up to 100 entries / 25MB) that records file read timestamps and content
- Make `FileReadTool` record reads into the cache
- Make `FileEditTool` and `FileWriteTool` check `cache.is_stale(path)` before writing, returning an error if the file was modified externally since last read
- Track `is_partial_view` for system-injected content (e.g., CLAUDE.md), requiring a full `FileRead` before editing

## Capabilities

### New Capabilities
- `trust-manager`: Directory trust verification system — checks, persists, and inherits trust status for project directories; gates execution of `apiKeyHelper` and `settings.json` env loading
- `file-state-cache`: LRU file state tracking — records file reads with timestamps and detects external modifications before file writes/edits

### Modified Capabilities
_(none — these are new subsystems with no existing specs)_

## Impact

- **core crate**: New `trust.rs` and `file_state_cache.rs` modules
- **cli crate**: `main.rs` startup flow modified to insert trust check before credential loading; new `--trust` CLI argument
- **tui crate**: New trust dialog component (similar pattern to existing permission dialog)
- **tools crate**: `FileReadTool`, `FileEditTool`, `FileWriteTool` modified to integrate with `FileStateCache`
- **Dependencies**: New `lru` crate dependency for the file state cache
- **Startup behavior change**: First run in a new directory will pause for trust confirmation (TUI) or reject (--print mode)
