## 1. SessionEvent Model (core crate)

- [x] 1.1 Define `SessionEvent` enum in `crates/core/src/session.rs` with `#[serde(tag = "type")]` — variants: `Header`, `UserMessage`, `AssistantMessage`, `CompactBoundary`, `UsageUpdate`, `PermissionChange`, `SessionEnd`
- [x] 1.2 Add `interrupted` field (`#[serde(default)]`) to `SessionSummary` struct
- [x] 1.3 Write unit tests for `SessionEvent` serde roundtrip (all variants serialize/deserialize correctly with type tags)

## 2. SessionWriter (cli crate)

- [x] 2.1 Create `SessionWriter` struct in `crates/cli/src/session.rs` with `BufWriter<File>` handle, `new(id, model, model_setting, cwd)` constructor that creates `.jsonl` file and writes `Header` event
- [x] 2.2 Implement `append_message(&mut self, msg: &Message)` — serializes as `UserMessage` or `AssistantMessage` based on role, writes line + flush
- [x] 2.3 Implement `append_event(&mut self, event: SessionEvent)` — serializes event, writes line + flush
- [x] 2.4 Implement `finish(&mut self)` — appends `SessionEnd` event with current timestamp, flushes, and closes
- [x] 2.5 Write unit tests for `SessionWriter`: create file, append messages, verify JSONL output line-by-line

## 3. SessionReader (cli crate)

- [x] 3.1 Implement `load_from_jsonl(path) -> Result<SessionFile>` — line-by-line parsing that reconstructs `SessionFile` from `SessionEvent` entries, skipping invalid lines with warnings
- [x] 3.2 Handle `CompactBoundary` during load: replace pre-boundary messages with summary message
- [x] 3.3 Handle `UsageUpdate` and `PermissionChange` during load: apply latest values to the reconstructed `SessionFile`
- [x] 3.4 Refactor `SessionFile::load()` into `load(path)` with auto-detection: `.jsonl` extension → `load_from_jsonl`, `.json` extension → existing JSON parser
- [x] 3.5 Write unit tests for `load_from_jsonl`: normal load, truncated last line recovery, CompactBoundary handling, unknown event type tolerance

## 4. Directory Scanning Updates (cli crate)

- [x] 4.1 Update `load_latest_session()` to scan for both `.json` and `.jsonl` files, preferring `.jsonl` when both exist for the same id
- [x] 4.2 Update `load_session_by_id(id)` to check for `{id}.jsonl` first, then `{id}.json`
- [x] 4.3 Update `list_recent_sessions_in_dir()` to include `.jsonl` files, detect interrupted sessions (missing `SessionEnd`), and set `interrupted` flag on `SessionSummary`
- [x] 4.4 Write unit tests for mixed-format directory scanning: listing, latest detection, format preference

## 5. Crash Recovery (cli crate)

- [x] 5.1 Add `is_interrupted(path) -> bool` helper that checks a `.jsonl` file for missing `SessionEnd` (read last line or scan for it)
- [x] 5.2 Update `load_latest_session()` / `--continue` logic to prefer the most recent interrupted session when one exists
- [x] 5.3 When resuming an interrupted session, open the existing `.jsonl` file in append mode so `SessionWriter` continues writing to it
- [x] 5.4 Write unit tests for crash recovery: interrupted detection, resume preference, append-to-existing behavior

## 6. Agent Loop Integration (sdk crate)

- [x] 6.1 Add an optional session writer parameter to `AgentLoop` (e.g., `Option<Arc<Mutex<SessionWriter>>>` or a trait object) that the CLI passes in
- [x] 6.2 After each agentic turn (post tool execution), call `append_message()` for the user message and assistant response, then `flush()`
- [x] 6.3 After compaction completes, write a `CompactBoundary` event via `append_event()`
- [x] 6.4 After permission mode/rules change, write a `PermissionChange` event via `append_event()`
- [x] 6.5 Write unit tests verifying the agent loop calls session writer methods at the correct points (use mock writer)

## 7. CLI Main Integration

- [x] 7.1 In `main.rs`, create `SessionWriter` when starting a new session or resuming an interrupted one, and pass it to `AgentLoop`
- [x] 7.2 Call `SessionWriter::finish()` on normal session termination (clean exit paths)
- [x] 7.3 Remove or gate the existing `SessionFile::save()` calls behind a legacy flag — new sessions use writer-based persistence
- [x] 7.4 Update `/compact` command to use `SessionWriter::append_event(CompactBoundary)` instead of full `SessionFile::save()`

## 8. Verification

- [x] 8.1 Run `cargo test --workspace` and fix any failures
- [x] 8.2 Manual test: start a new session, verify `.jsonl` file is created with correct events
- [x] 8.3 Manual test: kill a session mid-conversation, restart with `--continue`, verify recovery
- [x] 8.4 Manual test: verify old `.json` sessions still load and appear in session list
