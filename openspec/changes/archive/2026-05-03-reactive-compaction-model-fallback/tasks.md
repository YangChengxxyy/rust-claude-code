## 1. API Error Variants

- [x] 1.1 Add `PromptTooLong { message: String }` variant to `ApiError` in `crates/api/src/error.rs`
- [x] 1.2 Add `Overloaded { message: String }` variant to `ApiError` in `crates/api/src/error.rs`
- [x] 1.3 Update `map_error_response()` in `crates/api/src/client.rs` to detect HTTP 400 + `invalid_request_error` with prompt-too-long message patterns ("too long", "too many tokens", "exceeds the maximum") and map to `PromptTooLong`
- [x] 1.4 Update `map_error_response()` to detect HTTP 529 and map to `Overloaded`
- [x] 1.5 Add unit tests for `map_error_response()`: prompt-too-long detection, non-prompt 400 errors, HTTP 529, and existing 429 behavior unchanged

## 2. Fallback Model Configuration

- [x] 2.1 Add `fallback_model: Option<String>` field to `Config` struct in `crates/core/src/config.rs`
- [x] 2.2 Add `fallback_model: ConfigSource` to `ConfigProvenance` and `fallback_model: ResolvedField<Option<String>>` to `ConfigOverrides`
- [x] 2.3 Implement resolution chain: `RUST_CLAUDE_FALLBACK_MODEL` env → `settings.json` `fallbackModel` field → config.json → `None`
- [x] 2.4 Add unit tests for fallback model config resolution from each source

## 3. Micro-compaction

- [x] 3.1 Define `MicroCompactionResult` struct in `crates/sdk/src/compaction.rs` with fields: `blocks_cleared: usize`, `estimated_token_reduction: u32`
- [x] 3.2 Implement `CompactionService::micro_compact()` method that acquires the `AppState` mutex, iterates messages, and replaces `ToolResult` content blocks older than the preservation window (default 3 turns) with `[Content cleared to reduce context size]`
- [x] 3.3 Ensure micro-compaction targets Bash, FileRead, Grep, Glob, WebSearch, WebFetch tool results and skips already-cleared or empty blocks
- [x] 3.4 Add unit tests for micro-compaction: verify old tool results are cleared, recent ones preserved, text/thinking/tool-use blocks untouched, and idempotency on already-cleared blocks
- [x] 3.5 Mirror micro-compaction to `crates/cli/src/compaction.rs` (or refactor to share code)

## 4. RetryState and Reactive Compaction in Agent Loop

- [x] 4.1 Define `RetryState` struct in `crates/sdk/src/agent_loop.rs` with `prompt_too_long_stage: u8` and `consecutive_overload_count: u32`
- [x] 4.2 Instantiate `RetryState` at the start of `QueryLoop::run()`, reset `prompt_too_long_stage` on each new turn
- [x] 4.3 Add error-matching branch in the main loop for `QueryLoopError::Api(ApiError::PromptTooLong { .. })`: implement stage 1 (force_compact + retry), stage 2 (micro_compact + retry), stage 3 (report error)
- [x] 4.4 Add `OutputSink::compaction_start()` notification before stage 1 and error message before stage 2
- [x] 4.5 Add unit tests using `MockClient` to simulate `PromptTooLong` errors and verify three-stage escalation behavior

## 5. Model Fallback on Consecutive Overload

- [x] 5.1 Add error-matching branch in the main loop for `QueryLoopError::Api(ApiError::Overloaded { .. })`: increment `consecutive_overload_count`, apply backoff delay (1s * count)
- [x] 5.2 After exceeding `MAX_OVERLOAD_RETRIES` (3), check if `fallback_model` is configured; if yes, switch the model in `AppState` and notify user via `OutputSink::error()`
- [x] 5.3 Reset `consecutive_overload_count` to 0 on any successful API response
- [x] 5.4 Retry current turn immediately after model switch (no additional backoff)
- [x] 5.5 Add unit tests using `MockClient` to simulate consecutive 529 errors: verify counter increment, backoff, fallback switch, notification, and counter reset on success

## 6. CLI Agent Loop Sync

- [x] 6.1 Mirror the `RetryState` struct and reactive compaction logic to `crates/cli/src/query_loop.rs`
- [x] 6.2 Mirror the model fallback logic to `crates/cli/src/query_loop.rs`
- [x] 6.3 Ensure `TuiBridge` receives appropriate notifications for compaction and model switch events

## 7. Integration Verification

- [x] 7.1 Run `cargo check --workspace` and fix any compilation errors
- [x] 7.2 Run `cargo test --workspace` and ensure all existing and new tests pass
- [x] 7.3 Verify `cargo clippy --workspace` passes without new warnings
