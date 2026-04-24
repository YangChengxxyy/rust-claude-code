## Why

The TUI streaming rendering is the single biggest experience gap compared to the TypeScript reference. Currently, live streaming text is rendered as raw plain text with no markdown formatting, thinking content is hidden behind a spinner with no real-time visibility, tool call construction is invisible to the user, and `--print` mode dumps the full response only after completion rather than streaming token-by-token. These issues are noticeable on every single interaction. This iteration fixes the full streaming rendering pipeline — from API deltas through the TUI bridge to the screen — so that every phase of a response (text, thinking, tool calls) renders incrementally and fluidly.

## What Changes

- Implement incremental markdown rendering during streaming so headings, code blocks, lists, bold, and italic are formatted in real time rather than appearing as raw syntax until stream end.
- Stream thinking content in real time so the user sees the model's reasoning as it arrives instead of a static spinner, with collapsible fold/unfold support.
- Forward tool use `InputJsonDelta` events through the TUI bridge so tool call parameters appear incrementally as they are constructed.
- Optimize the rendering pipeline to avoid flicker and unnecessary full redraws during high-frequency delta events, ensuring sub-50ms single-frame latency.
- Enable token-by-token stdout streaming in `--print` mode so output appears progressively.
- Harden Ctrl+C / Esc interrupt handling during streaming so the terminal state is always cleanly restored.

## Capabilities

### New Capabilities
- `tui-streaming-markdown`: Incremental markdown parsing and rendering during live streaming — convert raw deltas into formatted ratatui output without waiting for the full message.
- `tui-thinking-streaming`: Real-time rendering of thinking/reasoning content during streaming with fold/unfold controls, replacing the current hidden-spinner approach.
- `tui-tool-streaming`: Incremental display of tool call construction (name, partial JSON input) during streaming so users see what tool the model is invoking before execution begins.
- `tui-render-optimization`: Rendering pipeline optimizations — dirty-region tracking, batched redraws, frame rate limiting — to eliminate flicker and maintain smooth scrolling at high delta rates.
- `cli-print-streaming`: Token-by-token stdout streaming for `--print` mode, replacing the current all-at-once dump.

### Modified Capabilities
- `tui-stream-controls`: Extend stream control handling for clean interrupt (Ctrl+C / Esc) during the new streaming phases (thinking, tool construction), ensuring terminal state cleanup.

## Impact

- `crates/tui`: Major changes — new incremental markdown parser, streaming thinking renderer, streaming tool call renderer, render pipeline optimization, dirty-region tracking.
- `crates/cli`: Changes to `query_loop.rs` (forward `InputJsonDelta` via bridge, expose streaming in `--print` mode), changes to `main.rs` (print-mode streaming loop).
- `crates/tui/src/bridge.rs`: New event types for tool input deltas.
- `crates/tui/src/events.rs`: New `AppEvent` variants for tool streaming.
- `crates/tui/src/app.rs`: New streaming state buffers for tool construction, thinking rendering state.
- `crates/tui/src/ui.rs`: Replace raw-text streaming render path with incremental markdown renderer, add thinking and tool streaming render paths.
- Performance: rendering must not introduce >50ms single-frame delay on long (>500 token) responses.
- No external dependency additions expected beyond what ratatui already provides.
