## 1. Trust Manager Core (core crate)

- [x] 1.1 Create `crates/core/src/trust.rs` with `TrustStatus` enum (`Trusted`, `Untrusted`, `InheritedFromParent`) and `TrustManager` struct
- [x] 1.2 Implement `TrustManager::check_trust(project_dir) -> TrustStatus` — load `trust.json`, canonicalize path, check exact match then parent cascading
- [x] 1.3 Implement `TrustManager::accept_trust(project_dir)` — persist to `~/.config/rust-claude-code/trust.json` with timestamp; skip persistence for home directory
- [x] 1.4 Implement home directory special handling — accept_trust for `~` sets in-memory flag only, not written to trust.json
- [x] 1.5 Write unit tests for TrustManager: exact match, parent inheritance, untrusted, home directory in-memory only, trust.json round-trip

## 2. CLI Trust Integration (cli crate)

- [x] 2.1 Add `--trust` flag to `Cli` struct in `main.rs` (clap argument)
- [x] 2.2 Implement trust gate in `main.rs` startup: after loading `ClaudeSettings` (user + project), check if project settings contain `apiKeyHelper` or `env`; if yes, run `TrustManager::check_trust(cwd)`
- [x] 2.3 Handle untrusted + `--print` mode: print error suggesting `--trust`, exit with non-zero status
- [x] 2.4 Handle untrusted + TUI mode: defer to TUI trust dialog (pass trust state to TUI app)
- [x] 2.5 Handle `--trust` flag: skip trust check entirely for current invocation, do not persist
- [x] 2.6 Handle trust denial: proceed with user-level settings only (skip project `apiKeyHelper` and `env`)
- [x] 2.7 Write integration test: verify `--trust` flag accepted by clap, verify trust gate logic with mock settings

## 3. TUI Trust Dialog (tui crate)

- [x] 3.1 Add `TrustDialog` struct to `app.rs` (project_path, response_tx oneshot channel) following the `PermissionDialog` pattern
- [x] 3.2 Add `AppEvent::TrustRequest` variant with project path and oneshot sender
- [x] 3.3 Implement `draw_trust_dialog()` in `ui.rs` — modal overlay showing project path, security warning about apiKeyHelper/env, Trust/Don't Trust options
- [x] 3.4 Implement `handle_trust_key()` in `app.rs` — `y` = Trust, `n`/`Esc` = Don't Trust, `Enter` selects current option
- [x] 3.5 Wire trust dialog into TUI bridge: expose method for CLI to request trust confirmation via `AppEvent::TrustRequest`
- [x] 3.6 Test TUI trust dialog rendering and key handling

## 4. File State Cache Core (core crate)

- [x] 4.1 Add `lru` crate dependency to `crates/core/Cargo.toml`
- [x] 4.2 Create `crates/core/src/file_state_cache.rs` with `FileState` struct (content_hash: u64, mtime: SystemTime, offset: Option<usize>, limit: Option<usize>, is_partial_view: bool, read_time: Instant)
- [x] 4.3 Implement `FileStateCache` struct wrapping `LruCache<PathBuf, FileState>` with capacity 100
- [x] 4.4 Implement `record_read(path, content, offset, limit, is_partial_view)` — compute content hash (use `std::hash::Hasher` with FxHash or DefaultHasher), stat mtime, insert into LRU
- [x] 4.5 Implement `is_stale(path) -> Option<bool>` — return `None` if no entry, `Some(false)` if mtime matches, `Some(true)` if mtime differs; apply hash fallback for same-second mtime edge case
- [x] 4.6 Implement `get_read_state(path) -> Option<&FileState>` for checking `is_partial_view`
- [x] 4.7 Implement `record_write(path, content)` — update cache entry after successful write with new hash and mtime
- [x] 4.8 Write unit tests: record_read + is_stale (not stale), is_stale (stale after external modification), LRU eviction at 100, partial_view flag, record_write updates entry

## 5. AppState Integration

- [x] 5.1 Add `file_state_cache: FileStateCache` field to `AppState` in `crates/core/src/state.rs`
- [x] 5.2 Initialize `FileStateCache::new(100)` in `AppState::new()`

## 6. Tool Integration (tools crate)

- [x] 6.1 Modify `FileReadTool::execute()` — after successful read, call `app_state.lock().file_state_cache.record_read(path, content, offset, limit, false)`
- [x] 6.2 Modify `FileEditTool::execute()` — before edit, check `is_stale(path)` and `is_partial_view`; if stale return error "File has been modified since last read. Please re-read the file before editing."; if partial_view return error "File was read as partial view (system-injected). Please read the file with FileRead before editing."
- [x] 6.3 Modify `FileEditTool::execute()` — after successful edit, call `record_write(path, new_content)`
- [x] 6.4 Modify `FileWriteTool::execute()` — before write, check `is_stale(path)` and `is_partial_view` (same logic as FileEdit)
- [x] 6.5 Modify `FileWriteTool::execute()` — after successful write, call `record_write(path, content)`
- [x] 6.6 Write tests for FileEditTool staleness rejection, FileWriteTool staleness rejection, partial_view rejection, and normal pass-through when not stale

## 7. System-Injected Content Tracking

- [x] 7.1 Identify where CLAUDE.md content is injected into system prompt (in sdk crate or cli crate) and add `file_state_cache.record_read(path, content, None, None, true)` for each auto-injected file
- [x] 7.2 Verify that a subsequent user-initiated FileRead of the same file clears the `is_partial_view` flag
- [x] 7.3 Write test: system-injected file is cached as partial_view, FileRead clears it, then FileEdit succeeds

## 8. Build Verification

- [x] 8.1 Run `cargo check --workspace` — ensure all crates compile cleanly
- [x] 8.2 Run `cargo test --workspace` — ensure all existing and new tests pass
- [x] 8.3 Run `cargo clippy --workspace` — ensure no new warnings
