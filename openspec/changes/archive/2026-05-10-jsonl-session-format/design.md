## Context

Sessions are persisted in `crates/cli/src/session.rs` as monolithic JSON files (`SessionFile::save()` calls `serde_json::to_string_pretty(self)`). The entire message history is serialized on every save. Currently, saves are triggered by the CLI binary in `main.rs` at two points: after `/compact` and after each TUI query loop run completes. Neither the SDK `agent_loop.rs` nor the CLI `query_loop.rs` performs any persistence internally — saving is the caller's responsibility.

Session files live at `~/.config/rust-claude-code/sessions/{id}.json`. Loading uses `serde_json::from_str` with backward-compatible `#[serde(default)]` fields. Helper functions (`load_latest_session`, `load_session_by_id`, `list_recent_sessions`) scan the directory for `.json` files.

The `AppState` holds the in-memory session state (messages, usage, permissions). The `SessionFile` struct is a persistence-focused snapshot that copies from `AppState` fields.

## Goals / Non-Goals

**Goals:**
- Replace full-file JSON rewrites with JSONL append-only writes for new sessions
- Enable crash recovery by detecting sessions that lack a `SessionEnd` marker
- Maintain backward compatibility: old `.json` sessions remain loadable
- Integrate incremental writes into the SDK agent loop so persistence happens per-turn rather than only at the CLI level
- Keep save latency constant regardless of conversation length (O(1) append vs O(n) rewrite)

**Non-Goals:**
- Migrating existing `.json` sessions to `.jsonl` format (they stay as-is, read-only compatible)
- Implementing automatic session compaction/rotation within JSONL files
- Adding WAL (write-ahead log) semantics or transactional guarantees beyond append+flush
- Implementing a TUI crash recovery dialog (this iteration adds the detection logic; TUI dialog is a future enhancement — for now, crash recovery uses CLI `--continue` behavior)

## Decisions

### 1. JSONL event-per-line format over binary or SQLite

**Choice:** One JSON object per line, newline-delimited.

**Rationale:** JSONL is human-readable, debuggable with standard tools (`cat`, `jq`), requires no new dependencies, and naturally supports append. SQLite would add a dependency and complexity for marginal benefit given our access patterns (append-only writes, full-scan reads on load). Binary formats sacrifice debuggability.

**Alternatives considered:**
- SQLite: Better for random access queries, but we only need sequential append and full-load. Adds `rusqlite` dependency.
- MessagePack/bincode: Smaller on disk but not human-readable; debugging session issues becomes harder.

### 2. `SessionEvent` as a tagged enum with `#[serde(tag = "type")]`

**Choice:** A single Rust enum with serde internal tagging, where each variant serializes as `{"type": "header", ...}`, `{"type": "user_message", ...}`, etc.

**Rationale:** Internal tagging produces self-describing JSON lines. The tag field enables format auto-detection (a line starting with `{"type":` is JSONL; a line starting with `{"id":` is legacy JSON). It also makes the format extensible — new event types can be added without breaking old readers (unknown types are skipped with a warning).

### 3. Writer lives in `cli` crate, called from SDK agent loop via trait

**Choice:** `SessionWriter` is defined in the `cli` crate alongside existing session code. The SDK agent loop receives an optional `Arc<Mutex<SessionWriter>>` (or a trait object) to call after each turn.

**Rationale:** Session persistence is currently a CLI concern. Moving it to SDK would require SDK to know about file paths and session directory conventions. Instead, CLI creates the writer and passes it into the agent loop. The agent loop calls `append_message` + `flush` at the end of each turn, and `append_event` for compaction boundaries and permission changes.

**Alternative considered:** Define a `SessionPersistence` trait in `core` and implement it in `cli`. This is cleaner but over-engineered for the current single-implementation case. Can be refactored later if needed.

### 4. Buffered writes with per-turn flush (no debounce initially)

**Choice:** Use `BufWriter<File>` with explicit `flush()` after each agentic turn. Skip the 100ms debounce described in the iteration plan for the initial implementation.

**Rationale:** The iteration plan mentions a debounce to avoid high-frequency small writes. However, agentic turns are inherently spaced (API call + tool execution takes seconds). Flushing once per turn is already low-frequency. Adding a debounce timer adds complexity (tokio timer, cancellation logic) with no practical benefit. If profiling shows flush overhead, debounce can be added later.

### 5. Format auto-detection by first byte

**Choice:** `SessionReader::load()` reads the first byte of the file. If it's `{`, treat as JSONL (each line is a JSON object starting with `{`). If it's something else or the file has a `.json` extension with a top-level object, fall back to legacy JSON parsing.

**Rationale:** Simple and reliable. JSONL files always start with a `{` (the header event). Legacy JSON files also start with `{` but are distinguished by file extension (`.json` vs `.jsonl`). The primary detection path is extension-based; first-byte check is a fallback for edge cases.

**Refined approach:** Check file extension first (`.jsonl` → JSONL parser, `.json` → legacy parser). For `load()` calls without extension hints, peek at the first line — if it contains `"type":`, use JSONL parser.

### 6. Crash recovery via missing `SessionEnd` marker

**Choice:** On startup, when listing sessions, check `.jsonl` files for a `SessionEnd` event in the last line. Sessions without it are marked as "interrupted." When `--continue` is used, prefer the most recent interrupted session. Display a note in the session list for interrupted sessions.

**Rationale:** This is a lightweight detection mechanism that doesn't require locking or PID files. False positives (session still running in another terminal) are possible but harmless — the user chooses whether to resume.

## Risks / Trade-offs

**[Concurrent writes from multiple terminals]** → If two processes open the same session and both append, the JSONL file could have interleaved lines. Mitigation: Session IDs are timestamp-based and unique per process. This is the same risk as the current JSON format (last writer wins). No change in risk profile.

**[Large JSONL files from very long sessions]** → Unlike JSON, we never rewrite the file, so it can only grow. A 1000-turn session with large tool results could produce a multi-MB file. Mitigation: `CompactBoundary` events mark points where earlier messages were summarized. Future optimization could truncate the file at compaction boundaries. For now, this matches the current JSON behavior (files also grow unbounded).

**[Partial last line on crash]** → If the process crashes mid-write, the last line could be truncated JSON. Mitigation: `SessionReader` skips lines that fail JSON parsing with a warning log. The session loads up to the last valid line.

**[File extension proliferation]** → Directory listing must now scan for both `.json` and `.jsonl`. Mitigation: Simple filter change from `ext == "json"` to `ext == "json" || ext == "jsonl"`. Minimal code impact.
