## 1. Chat scroll state and shortcuts

- [x] 1.1 Extend `crates/tui/src/app.rs` with chat viewport navigation state (including bottom-follow behavior) and helper methods to clamp scrolling against available content
- [x] 1.2 Add `PageUp`, `PageDown`, `Ctrl+Home`, and `Ctrl+End` handling for chat viewport navigation without breaking existing input editing and history shortcuts
- [x] 1.3 Ensure new messages, streaming deltas, thinking updates, and slash commands update the chat viewport according to whether the user is following the latest output

## 2. Shared rendering metrics

- [x] 2.1 Refactor `crates/tui/src/ui.rs` to expose shared chat line-building logic that both rendering and scroll-range calculation can use consistently
- [x] 2.2 Compute the maximum valid chat scroll offset from rendered line count and viewport height, and clamp offsets at both boundaries
- [x] 2.3 Update TUI help/status text and any related requirement-facing documentation to describe the actual chat scrolling shortcuts

## 3. Verification

- [x] 3.1 Add unit tests covering chat scroll boundaries, bottom-follow behavior, and preservation of the current input draft while scrolling history
- [x] 3.2 Run `cargo test -p rust-claude-tui`
- [x] 3.3 Manually run `cargo run -p rust-claude-cli` and verify long chat transcripts can be scrolled without regressing input editing or streaming display
