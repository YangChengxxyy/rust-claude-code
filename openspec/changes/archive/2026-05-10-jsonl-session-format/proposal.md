## Why

Sessions are currently stored as JSON snapshots (`serde_json::to_string_pretty`) that serialize the entire message history on every save. As conversations grow, save latency scales linearly with message count. Worse, if the process crashes or is killed between saves, the last interaction round is lost entirely because no partial write has occurred.

## What Changes

- Migrate session persistence from monolithic JSON to JSONL (JSON Lines) append-only log format
- Introduce `SessionWriter` for incremental, buffered append writes after each agentic turn
- Introduce `SessionReader` with auto-detection of `.jsonl` vs legacy `.json` formats
- Define a `SessionEvent` enum representing discrete log entries (header, messages, compact boundaries, usage updates, permission changes, session end)
- Add crash recovery: detect sessions missing a `SessionEnd` marker and offer to resume them
- Maintain full backward compatibility with existing `.json` session files

## Capabilities

### New Capabilities
- `jsonl-session-writer`: Incremental JSONL append writer with buffered I/O and per-turn flush, replacing the full-file JSON rewrite
- `jsonl-session-reader`: JSONL log parser that reconstructs `SessionFile` from event entries, with auto-detection to fall back to legacy JSON
- `session-event-model`: `SessionEvent` enum defining the structured log entry types (Header, UserMessage, AssistantMessage, CompactBoundary, UsageUpdate, PermissionChange, SessionEnd)
- `crash-recovery`: Detection of interrupted sessions (missing `SessionEnd`) and prompt to resume them on startup

### Modified Capabilities
- `session-resume`: Session loading now supports both `.jsonl` and `.json` formats; `list_recent_sessions()` scans both file types; `--continue` auto-recovers crashed sessions

## Impact

- **`cli` crate** — `session.rs` is the primary target: `SessionWriter`, `SessionReader`, format auto-detection, updated `sessions_dir()` scanning for dual extensions, crash detection logic
- **`sdk` crate** — `agent_loop.rs` gains post-turn calls to `SessionWriter::append_message()` + `flush()`, plus `CompactBoundary` and `PermissionChange` event writes
- **`tui` crate** — Crash recovery prompt dialog in `app.rs`
- **File format** — New sessions stored as `{id}.jsonl`; old `{id}.json` files remain readable
- **No new external dependencies** — uses existing `serde_json`, `tokio::io`, `chrono`
