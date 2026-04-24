## Context

The TUI currently handles streaming at the event level — `StreamDelta`, `ThinkingDelta`, etc. arrive through the `TuiBridge` and accumulate in `App` state buffers (`streaming_text`, `streaming_thinking`). However, the rendering path treats live streaming text as raw plain text (no markdown parsing) and hides thinking content behind a spinner. Tool call construction (`InputJsonDelta`) is silently accumulated in `query_loop.rs` and never forwarded to the TUI. `--print` mode collects the full response before outputting anything.

The ratatui immediate-mode rendering model means every frame rebuilds the visible area. The current `build_chat_lines()` function already handles this, but streaming deltas at high frequency (10-50 events/second) can cause visual flicker if every delta triggers a full re-layout of the chat area.

Key files: `crates/tui/src/ui.rs` (rendering), `crates/tui/src/app.rs` (state + event handling), `crates/tui/src/events.rs` (event types), `crates/tui/src/bridge.rs` (bridge), `crates/cli/src/query_loop.rs` (delta forwarding), `crates/cli/src/main.rs` (print mode).

## Goals / Non-Goals

**Goals:**
- Markdown formatting visible during streaming (headings, code blocks, lists, inline styles)
- Thinking content visible in real time with fold/unfold
- Tool call parameters visible as they are constructed
- No flicker or visible stutter during high-frequency deltas
- `--print` mode streams token-by-token to stdout
- Clean terminal state after Ctrl+C / Esc in all streaming phases

**Non-Goals:**
- Syntax highlighting inside code blocks (planned for iteration 24)
- Table, blockquote, or link rendering in markdown (future enhancement)
- Animated fold/unfold transitions for thinking blocks (simple toggle is sufficient)
- Streaming audio or image content
- Sub-character rendering (we work at the line level, not character level)

## Decisions

### D1: Incremental markdown parsing approach

**Decision**: Use a line-oriented incremental parser that maintains a small state machine (current block type: paragraph, code block, list, heading) and processes each new line of `streaming_text` as it arrives. The parser appends styled `ratatui::Line` items to a growing `Vec<Line>` that the renderer uses directly.

**Rationale**: The existing `parse_markdown_blocks()` function re-parses the entire text every frame. For streaming, we need to avoid O(n) re-parsing where n is the total accumulated text. A line-oriented approach works because markdown's block structure is determined line-by-line (a heading is one line, a code fence is one line, etc.). We track the parser state (are we inside a code block?) so new lines can be classified immediately.

**Alternatives considered**:
- *Full re-parse on every frame*: Simpler but O(n) per frame. At 500+ tokens, this becomes measurably slow (~5ms per parse). Rejected for performance.
- *Character-level incremental parser*: More granular but significantly more complex, especially for code fence detection and inline span closing. Line-level granularity is sufficient since ratatui renders line-by-line anyway.

### D2: Streaming text buffer structure

**Decision**: Replace the current `streaming_text: String` buffer with a `StreamingMarkdownState` struct that holds:
- `lines_cache: Vec<Line<'static>>` — already-parsed lines ready for rendering
- `pending_line: String` — the current incomplete line (no `\n` received yet)
- `parser_state: BlockState` — enum tracking whether we're inside a code block (and language), list, or paragraph context

On each `StreamDelta(text)`, we split the text by newlines. Complete lines go through the block parser and their styled `Line` output is appended to `lines_cache`. The last fragment (if no trailing newline) goes to `pending_line`. On each render frame, we display `lines_cache` + a live rendering of `pending_line`.

**Rationale**: This avoids re-parsing completed lines and keeps the renderer O(1) per delta (plus the cost of the one pending line). The `pending_line` handles the common case where a token arrives mid-word.

### D3: Thinking content real-time rendering

**Decision**: Show thinking content in real-time during streaming, rendered in a distinct dim/italic style to visually separate it from the main response. A `[Thinking]` header line precedes the thinking text. Once the thinking block completes (`ThinkingComplete`), the block converts to the existing collapsible `ChatMessage::Thinking` with fold/unfold support.

During streaming, thinking text uses the same incremental line cache approach as D2 but with a separate `streaming_thinking_lines: Vec<Line>` and `pending_thinking_line: String`.

**Rationale**: Hiding thinking behind a spinner wastes valuable transparency. Users benefit from seeing reasoning in real time. Using the same incremental approach avoids code duplication.

**Alternatives considered**:
- *Keep spinner, show on completion only*: Current behavior. Rejected because real-time visibility is the core UX improvement.
- *Inline thinking in the same stream as response text*: Confusing — the model may think then respond, and interleaving makes it hard to distinguish. Separate visual block is clearer.

### D4: Tool call streaming via new event types

**Decision**: Add a new `ToolInputDelta { name: String, json_fragment: String }` variant to `AppEvent`. In `query_loop.rs`, forward `InputJsonDelta` events through the bridge. In `App`, accumulate fragments in a `streaming_tool_input: Option<StreamingToolState>` that holds the tool name and accumulated JSON string. Render this as a tool call card with the tool name header and the partial JSON below it, styled as a code block.

When the tool use block completes (at `ContentBlockStop`), send `ToolUseStart` as before (with the complete input) and clear the streaming tool state.

**Rationale**: Tool calls can take 2-5 seconds to construct. Showing the JSON as it builds gives users immediate feedback about what the model is doing. The existing `ToolUseStart` event remains the authoritative "tool is ready" signal — the streaming display is purely for progress visibility.

**Alternatives considered**:
- *Show only tool name, not partial JSON*: Too little information. The JSON shows what file, what command, etc.
- *Parse and pretty-print partial JSON*: Risky — partial JSON is not valid JSON. Displaying raw fragments is safer and still informative.

### D5: Render pipeline optimization

**Decision**: Implement a frame-rate limiter that caps TUI redraws at ~30 FPS (one frame per ~33ms). When a `StreamDelta` arrives, mark the chat area as dirty but don't redraw immediately. A separate tick timer (33ms interval) triggers the actual redraw if the dirty flag is set. This batches multiple rapid deltas into a single frame.

Additionally, the `build_chat_lines()` function will take advantage of the line cache from D2 — it only needs to rebuild lines for the pending line and the scroll viewport calculation, not re-parse all historical messages.

**Rationale**: Without frame limiting, each delta event triggers a full terminal redraw. At 30-50 deltas/second, this causes visible flicker (terminal cursor jumps, partial frames). 30 FPS is the sweet spot: fast enough to feel real-time, slow enough to batch deltas. The original Claude Code uses a similar approach.

**Alternatives considered**:
- *No frame limiting, optimize render path only*: Still flickers because terminal I/O itself has cost. Even with O(1) state updates, 50 `stdout.flush()` per second causes visual noise.
- *Higher FPS cap (60)*: Diminishing returns in a terminal. Terminals typically refresh at 30-60 Hz. 30 FPS is sufficient and halves the render work.

### D6: `--print` mode streaming

**Decision**: In `--print` mode, create a lightweight stdout bridge (not a full TUI) that receives `StreamDelta` events and writes them directly to stdout with `io::Write::flush()`. No markdown rendering — raw text, matching the original Claude Code `--print` behavior. `ThinkingDelta` events are suppressed in print mode (thinking is not shown in non-interactive output). `ToolUseStart` / `ToolResult` are formatted as brief log lines to stderr.

**Rationale**: `--print` is used for piping output to other tools. Raw text is the expected format. Thinking is noise for machine consumption. Tool events go to stderr so they don't pollute the piped stdout.

### D7: Interrupt handling during new streaming phases

**Decision**: Extend the existing Ctrl+C / Esc cancellation to cover all streaming phases (thinking streaming, tool input streaming). When the user cancels:
1. Send `CancelStream` to the query loop (existing behavior)
2. Finalize any in-progress streaming state: convert `streaming_thinking_lines` to a `ChatMessage::Thinking` with a "(cancelled)" note, clear `streaming_tool_input`
3. Ensure the input area is re-enabled

No changes to the terminal cleanup path (`TerminalGuard`) since it already handles raw mode restoration.

**Rationale**: The cancel path must account for new streaming states to avoid orphaned buffers or stuck UI states.

## Risks / Trade-offs

- **[Line-level markdown may split inline spans]** A bold span `**word**` that arrives as two deltas (`**wo` + `rd**`) won't be detected until the full line is available. → Mitigation: The `pending_line` buffer defers inline parsing until a newline arrives. This means inline formatting appears slightly delayed (by one line) but is always correct. Acceptable because most deltas include enough text for line boundaries.

- **[Partial JSON display may confuse users]** Showing incomplete JSON like `{"command": "git st` could be misread. → Mitigation: Clearly label the display as "constructing..." with a visual indicator (spinner or `...` suffix). The partial JSON is styled distinctly from completed tool results.

- **[30 FPS cap may feel sluggish on fast terminals]** Some users on modern terminals (kitty, WezTerm) may expect higher refresh. → Mitigation: 30 FPS is imperceptible for text streaming. If needed, the cap can be made configurable later. The original Claude Code effectively operates at this range.

- **[State machine complexity in incremental parser]** The block-level state machine (tracking code fences, list context) must be correct across delta boundaries. → Mitigation: Comprehensive unit tests for the parser state machine with split-point edge cases. The state is simple (3-4 states) so enumeration of transitions is feasible.

- **[Memory growth from line cache]** `lines_cache` grows unboundedly during a long response. → Mitigation: This matches the current behavior where `streaming_text` grows unboundedly. After `StreamEnd`, the cache is discarded and replaced by a `ChatMessage::Assistant`. For extremely long responses, compaction already addresses context growth.
