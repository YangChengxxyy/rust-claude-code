## 1. Streaming Markdown Parser

- [x] 1.1 Create `StreamingMarkdownState` struct in `crates/tui/src/` with `lines_cache: Vec<Line<'static>>`, `pending_line: String`, and `BlockState` enum (Paragraph, CodeBlock { lang }, List { ordered }) — implement `push_delta(&mut self, text: &str)` that splits by newlines, parses complete lines through the block state machine, and appends styled `Line` items to `lines_cache`
- [x] 1.2 Implement block-level line classifier: detect heading lines (`# `, `## `, `### `), code fence open/close (` ``` `), unordered list items (`- `, `* `), ordered list items (`N. `), and plain paragraph lines — apply appropriate ratatui styles using existing `theme.rs` constants
- [x] 1.3 Implement inline span parser for complete lines: detect and style `` `code` ``, `**bold**`, `*italic*` spans within a line — produce `Vec<Span>` for each line
- [x] 1.4 Implement `render_pending_line(&self) -> Line` that applies best-effort inline formatting to the incomplete pending line buffer
- [x] 1.5 Add unit tests for `StreamingMarkdownState`: verify heading detection, code block state across deltas, list rendering, inline spans, pending line handling, and line cache growth — run `cargo test -p rust-claude-tui`

## 2. TUI Event Types and Bridge Extensions

- [x] 2.1 Add `ToolInputStreamStart { name: String }` and `ToolInputDelta { name: String, json_fragment: String }` variants to `AppEvent` in `crates/tui/src/events.rs`
- [x] 2.2 Add `send_tool_input_stream_start(&self, name: &str)` and `send_tool_input_delta(&self, name: &str, fragment: &str)` methods to `TuiBridge` in `crates/tui/src/bridge.rs`
- [x] 2.3 Run `cargo test -p rust-claude-tui` to verify event type and bridge compile correctly

## 3. App State for Streaming Phases

- [x] 3.1 Replace `streaming_text: String` in `App` with `streaming_md: StreamingMarkdownState` — update `handle_app_event` for `StreamDelta` to call `streaming_md.push_delta(text)` instead of string append
- [x] 3.2 Add `streaming_thinking_md: StreamingMarkdownState` and `thinking_folded: bool` fields to `App` — update `ThinkingStart`/`ThinkingDelta` handling to push deltas into the thinking markdown state and render in real time
- [x] 3.3 Add `streaming_tool: Option<StreamingToolState>` field to `App` where `StreamingToolState { name: String, accumulated_json: String }` — handle `ToolInputStreamStart` and `ToolInputDelta` events to accumulate partial JSON
- [x] 3.4 Update `StreamEnd` handling to finalize all three streaming states: convert `streaming_md` to `ChatMessage::Assistant`, `streaming_thinking_md` to `ChatMessage::Thinking`, clear `streaming_tool`
- [x] 3.5 Add Tab key handling during streaming to toggle `thinking_folded` state
- [x] 3.6 Run `cargo test -p rust-claude-tui` and `cargo check -p rust-claude-tui` to verify state management compiles and existing tests pass

## 4. Rendering Pipeline Updates

- [x] 4.1 Update `build_chat_lines()` in `ui.rs` to use `streaming_md.lines_cache` + `streaming_md.render_pending_line()` instead of raw `streaming_text` plain-text rendering
- [x] 4.2 Add thinking streaming render path in `build_chat_lines()`: render `[Thinking]` header in dim style, then `streaming_thinking_md.lines_cache` + pending line (unless `thinking_folded`, in which case show only header with "..." indicator)
- [x] 4.3 Add tool streaming render path in `build_chat_lines()`: when `streaming_tool` is `Some`, render a tool card with tool name header and accumulated partial JSON in code block style with "constructing..." label
- [x] 4.4 Update auto-scroll logic in `sync_chat_viewport()` to account for new streaming content types (thinking lines, tool card lines) when computing total content height
- [x] 4.5 Run `cargo check -p rust-claude-tui` and verify rendering compiles

## 5. Frame Rate Limiting and Render Optimization

- [x] 5.1 Add a 33ms tick timer to the TUI event loop alongside keyboard and app events — implement dirty flag tracking: set dirty on `StreamDelta`/`ThinkingDelta`/`ToolInputDelta`, only call `terminal.draw()` on tick if dirty or on non-streaming events immediately
- [x] 5.2 Optimize `build_chat_lines()` to skip re-computing status bar and input area layouts when only the chat region is dirty (pass a `dirty_regions` hint)
- [x] 5.3 Add performance assertion test: create a `StreamingMarkdownState`, push 500 lines, measure `render_pending_line()` + cache access time is under 50ms
- [x] 5.4 Run `cargo test -p rust-claude-tui` to verify optimization changes pass

## 6. QueryLoop Delta Forwarding

- [x] 6.1 In `crates/cli/src/query_loop.rs` `collect_response_from_stream()`, forward `ContentBlockStart` for `tool_use` blocks as `bridge.send_tool_input_stream_start(name)` — forward `InputJsonDelta` as `bridge.send_tool_input_delta(name, fragment)`
- [x] 6.2 Ensure `ToolUseStart` event is still sent with complete input after `ContentBlockStop` for tool_use blocks (existing behavior preserved)
- [x] 6.3 Run `cargo test -p rust-claude-cli` and `cargo check -p rust-claude-cli` to verify forwarding compiles and existing tests pass

## 7. Print Mode Streaming

- [x] 7.1 In `crates/cli/src/main.rs`, create a `PrintBridge` struct (or lightweight channel consumer) that receives `StreamDelta` events and writes each to stdout with immediate flush — replace the current all-at-once output in `--print` mode
- [x] 7.2 Suppress `ThinkingDelta` events in print mode — write `ToolUseStart`/`ToolResult` summaries to stderr as brief log lines
- [x] 7.3 Add Ctrl+C signal handler for print mode: flush stdout, write newline, exit with status code 130
- [x] 7.4 Run `cargo check -p rust-claude-cli` and verify print mode compiles

## 8. Interrupt Handling Extensions

- [x] 8.1 Update `CancelStream` / Escape handling in `App` to finalize `streaming_thinking_md` as `ChatMessage::Thinking { summary: "... (cancelled)", .. }` and clear `streaming_tool` state
- [x] 8.2 Verify input area re-enables after cancel in all streaming phases (text, thinking, tool input)
- [x] 8.3 Add unit tests for cancel during each streaming phase: cancel during text streaming, cancel during thinking streaming, cancel during tool input streaming — verify no orphaned state remains
- [x] 8.4 Run `cargo test -p rust-claude-tui` to verify cancel handling

## 9. Integration Verification

- [x] 9.1 Run `cargo test --workspace` to verify all crates pass
- [x] 9.2 Run `cargo build` to verify the full workspace compiles without warnings
- [x] 9.3 Manual smoke test: run `cargo run -p rust-claude-cli` in TUI mode, send a prompt that triggers streaming with thinking and tool calls, verify markdown renders incrementally, thinking is visible, tool inputs stream, and cancel works cleanly
- [x] 9.4 Manual smoke test: run `cargo run -p rust-claude-cli -- --print "hello"` and verify tokens stream to stdout progressively
