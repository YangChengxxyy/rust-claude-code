## 1. Core Types (core crate)

- [x] 1.1 Define `HookEvent` enum (`PreToolUse`, `PostToolUse`, `UserPromptSubmit`, `Stop`, `Notification`) with string serialization/deserialization in `core/src/hooks.rs`
- [x] 1.2 Define `HookConfig` struct (`type_`, `command`, `matcher`, `timeout`) with serde deserialization compatible with TS settings.json format
- [x] 1.3 Define `HookEventGroup` struct (`matcher: Option<String>`, `hooks: Vec<HookConfig>`) and the top-level `HooksConfig` type alias (`HashMap<String, Vec<HookEventGroup>>`)
- [x] 1.4 Define `HookResult` enum (`Continue`, `Block { reason: String }`) for PreToolUse decision results
- [x] 1.5 Define hook input structs: `BaseHookInput`, `PreToolUseInput`, `PostToolUseInput`, `UserPromptSubmitInput`, `StopInput`, `NotificationInput` with serde Serialize
- [x] 1.6 Add unit tests for HookEvent serialization, HookConfig/HookEventGroup deserialization, and HookResult types
- [x] 1.7 Export hooks module from `core/src/lib.rs`, verify `cargo test -p rust-claude-core` passes

## 2. Settings Integration (core crate)

- [x] 2.1 Add `hooks: HashMap<String, Vec<HookEventGroup>>` field to `ClaudeSettings` with `#[serde(default)]`
- [x] 2.2 Update `ClaudeSettings::merge()` to concatenate hook lists per event (project appended after user)
- [x] 2.3 Add unit tests for hooks field loading and merging across user/project settings layers
- [x] 2.4 Verify `cargo test -p rust-claude-core` passes with settings changes

## 3. Hook Execution Engine (cli crate)

- [x] 3.1 Create `cli/src/hooks.rs` with `HookRunner` struct holding `HooksConfig` and `cwd: PathBuf`
- [x] 3.2 Implement `HookRunner::get_matching_hooks(event, tool_name)` — filter hook groups by event and matcher
- [x] 3.3 Implement `HookRunner::execute_command_hook(config, input_json)` — spawn shell process, write stdin, capture stdout/stderr, enforce timeout via `tokio::time::timeout`
- [x] 3.4 Implement `HookRunner::parse_pre_tool_use_result(stdout, exit_code)` — parse JSON decision or use exit code semantics (0=approve, 1=warn, 2=block)
- [x] 3.5 Implement `HookRunner::run_pre_tool_use(tool_name, tool_input, session_id)` — orchestrate matching, execution, and result aggregation for PreToolUse event; short-circuit on first block
- [x] 3.6 Implement `HookRunner::run_post_tool_use(tool_name, tool_input, tool_output, is_error, session_id)` — fire-and-forget PostToolUse hooks
- [x] 3.7 Implement `HookRunner::run_user_prompt_submit(user_message, session_id)` — fire UserPromptSubmit hooks
- [x] 3.8 Implement `HookRunner::run_stop(stop_reason, session_id)` — fire Stop hooks
- [x] 3.9 Add unit tests for matcher filtering, result parsing (approve/block/empty/invalid JSON/exit codes), and timeout behavior
- [x] 3.10 Export hooks module from `cli/src/lib.rs`, verify `cargo test -p rust-claude-cli` passes

## 4. QueryLoop Integration (cli crate)

- [x] 4.1 Add `hook_runner: Option<Arc<HookRunner>>` field to `QueryLoop` and `with_hook_runner()` builder method
- [x] 4.2 Integrate PreToolUse hooks in `execute_tool_uses()`: after permission check, before tool execution; on block, skip execution and add error tool result
- [x] 4.3 Handle PreToolUse hooks for concurrent tool batch: evaluate hooks for each tool before parallel execution, skip blocked tools
- [x] 4.4 Integrate PostToolUse hooks in `execute_tool_uses()`: after each tool completes, fire hooks (non-blocking)
- [x] 4.5 Integrate UserPromptSubmit hooks in `run()`: before building the API request, fire hooks with the user message
- [x] 4.6 Integrate Stop hooks: after the main loop exits, fire hooks with the stop reason
- [x] 4.7 Add unit tests using `MockClient` verifying hook block prevents tool execution and hook approve allows it
- [x] 4.8 Verify `cargo test -p rust-claude-cli` passes

## 5. TUI Integration (tui crate)

- [x] 5.1 Add `AppEvent::HookBlocked { tool_name: String, reason: String }` event variant
- [x] 5.2 Add `TuiBridge::send_hook_blocked(tool_name, reason)` method
- [x] 5.3 Handle `HookBlocked` event in TUI app: display as system message in chat area
- [x] 5.4 Wire `send_hook_blocked` calls in QueryLoop when a PreToolUse hook blocks a tool
- [x] 5.5 Verify `cargo test -p rust-claude-tui` passes

## 6. Slash Command & Startup Wiring

- [x] 6.1 Implement `/hooks` slash command: display configured hooks grouped by event, showing matcher and command
- [x] 6.2 Wire hook config loading in `main.rs`: load from merged settings, construct `HookRunner`, pass to `QueryLoop`
- [x] 6.3 Pass `HookRunner` to both TUI mode and print mode execution paths
- [x] 6.4 Verify `cargo test --workspace` passes

## 7. Final Verification

- [x] 7.1 Run `cargo build` to verify clean compilation across all crates
- [x] 7.2 Run `cargo test --workspace` to verify all tests pass
- [x] 7.3 Manual smoke test: configure a PreToolUse hook in settings.json that blocks `Bash(rm *)`, verify it prevents dangerous commands
- [x] 7.4 Update `doc/requirement.md` iteration 18 status to "已完成" with completion notes
