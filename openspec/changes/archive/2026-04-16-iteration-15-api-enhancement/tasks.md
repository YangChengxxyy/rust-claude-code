## 1. Core Types: Thinking Block & Forward Compatibility

- [x] 1.1 Add `signature: Option<String>` field to `ContentBlock::Thinking` in `core/src/message.rs` with `#[serde(default, skip_serializing_if = "Option::is_none")]`; update `ContentBlock::thinking()` constructor; add `Unknown` variant with `#[serde(other)]` for forward-compatible deserialization
- [x] 1.2 Add `ThinkingConfig` enum (`Disabled`, `Enabled { budget_tokens: u32 }`, `Adaptive`) to `core` crate with API-compatible serialization; add `thinking_enabled: bool` (default true) to `SessionSettings`
- [x] 1.3 Add unit tests: Thinking block round-trip with/without signature, Unknown block deserialization, ThinkingConfig serialization variants, backward compat with legacy session files
- [x] 1.4 Run `cargo test -p rust-claude-core` — verify all tests pass

## 2. API Types: Cache Control & Thinking Request

- [x] 2.1 Add `CacheControl` struct and `SystemBlock` struct to `api/src/types.rs`; add `SystemPrompt::StructuredBlocks(Vec<SystemBlock>)` variant; add builder method `SystemBlock::with_cache_control()`
- [x] 2.2 Add `thinking: Option<ThinkingConfig>` field to `CreateMessageRequest` with `#[serde(skip_serializing_if = "Option::is_none")]`; add `.with_thinking()` builder method
- [x] 2.3 Add `inject_cache_control_on_messages()` function in `api/src/types.rs` that takes `&mut Vec<serde_json::Value>` (serialized messages) and injects `cache_control` on the last content block of the last message
- [x] 2.4 Add unit tests: SystemBlock serialization with/without cache_control, StructuredBlocks serialization, CreateMessageRequest with thinking field, inject_cache_control_on_messages on various message shapes
- [x] 2.5 Run `cargo test -p rust-claude-api` — verify all tests pass

## 3. Token Counting: Usage-Based Estimation

- [x] 3.1 Add `last_api_usage: Option<Usage>` and `last_api_message_index: usize` fields to `AppState` in `core/src/state.rs`; add `update_api_usage()` method that records usage and current message count
- [x] 3.2 Add `estimate_current_tokens()` function in `core/src/compaction.rs` that implements the two-tier counting: `last_api_input_tokens + estimate_tokens(messages[last_index..])` with chars/4 fallback for first turn
- [x] 3.3 Update `needs_compaction()` to accept optional `Usage` reference and use `estimate_current_tokens()` instead of pure chars/4
- [x] 3.4 Add unit tests: estimate_current_tokens with/without API usage, needs_compaction with usage-based counting, edge case of first turn fallback
- [x] 3.5 Run `cargo test -p rust-claude-core` — verify all tests pass

## 4. QueryLoop: Thinking & Caching Integration

- [x] 4.1 Add `model_supports_thinking()` and `get_thinking_config_for_model()` helper functions in `cli/src/query_loop.rs` that auto-detect adaptive/budget/disabled based on model string
- [x] 4.2 Update `build_request()` to: (a) convert system prompt to `StructuredBlocks` with `cache_control` on last block; (b) inject thinking config based on model and `session.thinking_enabled`; (c) serialize messages as `serde_json::Value` and call `inject_cache_control_on_messages()`
- [x] 4.3 Update `collect_response_from_stream()` and the non-streaming path to call `app_state.update_api_usage()` with the response Usage data
- [x] 4.4 Add unit tests with MockClient: verify thinking config is set correctly for different models, cache_control appears in serialized requests, usage tracking updates AppState
- [x] 4.5 Run `cargo test -p rust-claude-cli` — verify all tests pass

## 5. QueryLoop: Max Tokens Recovery

- [x] 5.1 Add `max_tokens_recovery_count: usize` field to QueryLoop run state; add `MAX_TOKENS_RECOVERY_LIMIT: usize = 3` constant
- [x] 5.2 Modify the main loop in `run()`: after receiving a response with `stop_reason == MaxTokens`, add truncated assistant message to state, inject continuation user message, increment recovery counter, and continue loop (respecting max_rounds)
- [x] 5.3 Handle edge case: if truncated response has tool_use blocks, execute them before injecting continuation (per spec)
- [x] 5.4 Notify TUI bridge of recovery in progress (send status update with recovery attempt info)
- [x] 5.5 Add unit tests with MockClient: recovery succeeds after one retry, recovery exhausts limit, recovery counter resets between user messages, tool_use in truncated response handled correctly
- [x] 5.6 Run `cargo test -p rust-claude-cli` — verify all tests pass

## 6. CLI: New Parameters

- [x] 6.1 Add `--thinking` and `--no-thinking` CLI flags to `Cli` struct in `main.rs`; wire them into `SessionSettings.thinking_enabled`
- [x] 6.2 Verify the full config priority chain works: CLI flags override settings.json which override defaults
- [x] 6.3 Run `cargo test -p rust-claude-cli` — verify all tests pass

## 7. TUI: Thinking Display & Cache Info

- [x] 7.1 Update `ui.rs` to render thinking blocks: during streaming show "Thinking..." spinner; after completion show collapsed summary line "Thought for N tokens"
- [x] 7.2 Add cache hit display to status bar: show `cache_read_input_tokens / input_tokens` ratio from latest usage; update `bridge.rs` to carry cache info in `UsageUpdate` event
- [x] 7.3 Add recovery status display: show "Continuing... (attempt N/3)" when max tokens recovery is active
- [x] 7.4 Run `cargo test -p rust-claude-tui` — verify all tests pass
- [x] 7.5 Fix duplicate assistant messages in streaming TUI by avoiding a second `AssistantMessage` dispatch after `StreamEnd`; verify with `cargo test -p rust-claude-cli -p rust-claude-tui`

## 8. Compaction Integration & Auto-Compact

- [x] 8.1 Update `CompactionService::compact_if_needed()` to use the new `estimate_current_tokens()` function (usage-based) instead of the old pure-heuristic `needs_compaction()`
- [x] 8.2 Ensure compaction summary request also uses thinking config and cache_control where appropriate
- [x] 8.3 Add integration tests: auto-compact triggers correctly with usage-based counting, compact request includes appropriate API features
- [x] 8.4 Run `cargo test -p rust-claude-cli` — verify all tests pass

## 9. End-to-End Verification

- [x] 9.1 Run `cargo test --workspace` — verify all crate tests pass together
- [x] 9.2 Run `cargo check --workspace` — verify no warnings
- [x] 9.3 Manual integration test: start a conversation with `cargo run -p rust-claude-cli`, verify (a) cache_read_input_tokens > 0 on second turn, (b) thinking blocks appear in output, (c) status bar shows cache info
