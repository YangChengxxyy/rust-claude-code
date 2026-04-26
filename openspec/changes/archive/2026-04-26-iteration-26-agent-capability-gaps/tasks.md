## 1. Bash Persistent CWD

- [x] 1.1 Add focused tests describing session CWD defaults, explicit `workdir`, successful `cd`, nonzero-exit CWD update, timeout no-update, and invalid-directory no-update behavior
- [x] 1.2 Update `BashTool` to choose its starting directory from explicit `workdir` or `AppState.cwd`
- [x] 1.3 Capture the shell's final working directory separately from visible stdout/stderr output
- [x] 1.4 Update `AppState.cwd` after completed Bash commands when the final directory is valid
- [x] 1.5 Ensure TUI status/system behavior remains coherent when CWD changes, including hooks receiving the updated CWD on later tool calls

## 2. AskUserQuestion Tool

- [x] 2.1 Define AskUserQuestion input/response types with `question`, `options`, `allow_custom`, selected label, and answer text
- [x] 2.2 Extend `ToolContext` with an optional user-question callback that tools can call without depending on the TUI crate
- [x] 2.3 Implement `AskUserQuestionTool` validation, schema, execution, cancellation/unavailable handling, and deterministic non-interactive fallback
- [x] 2.4 Register `AskUserQuestionTool` in the default tool registry and include it in tool descriptions
- [x] 2.5 Add TUI `AppEvent`/bridge support for user-question requests with a oneshot response channel
- [x] 2.6 Add TUI modal state, rendering, keyboard navigation, custom input handling, submit, and cancel behavior
- [x] 2.7 Add tests for option selection, custom answers, cancellation, fallback behavior, missing callback, and invalid input

## 3. WebSearch Real Backend

- [x] 3.1 Decide the first real provider implementation based on API simplicity and available configuration shape, preferring Brave Search unless implementation evidence points elsewhere
- [x] 3.2 Add WebSearch provider configuration via environment/settings while keeping secrets out of persisted non-secret config where possible
- [x] 3.3 Implement a real `SearchBackend` that maps provider responses into `SearchResult`
- [x] 3.4 Update `WebSearchTool::new` or add a configured constructor so CLI startup can inject the selected backend
- [x] 3.5 Preserve existing domain filtering, no-results behavior, and formatted output
- [x] 3.6 Add fake-backend/unit tests for success, empty results, provider errors, malformed responses, missing credentials, and domain filters
- [x] 3.7 Add an ignored/env-gated live provider test or manual verification note

## 4. Integration and Verification

- [x] 4.1 Run `cargo fmt --all`
- [x] 4.2 Run `cargo test -p rust-claude-tools`
- [x] 4.3 Run `cargo test -p rust-claude-cli`
- [x] 4.4 Run `cargo test -p rust-claude-tui`
- [x] 4.5 Run `cargo test --workspace`
- [ ] 4.6 Manual test: in TUI, run a prompt that triggers Bash `cd /tmp && pwd`, then a later Bash `pwd`, and verify the second command starts in `/tmp`
- [ ] 4.7 Manual test: in TUI, trigger AskUserQuestion, choose an option, and verify the model receives the selected answer
- [ ] 4.8 Manual test: configure WebSearch credentials, run a search, and verify real results plus allowed/blocked domain filtering
