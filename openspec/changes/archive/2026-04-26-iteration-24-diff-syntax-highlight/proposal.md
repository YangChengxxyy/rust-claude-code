## Why

When the model calls FileEdit or FileWrite, the TUI permission dialog shows only a one-line summary like `"src/main.rs (edit)"` — the user must approve or deny a code change they cannot see. This is the most critical safety and UX gap remaining after iteration 23: every write operation forces a blind trust decision. Additionally, all code blocks (in assistant messages and tool results) render as monochrome text with no syntax highlighting, making code review inside the TUI significantly harder than it needs to be. Addressing both issues in one iteration is natural because diff preview rendering benefits directly from the same syntax highlighting infrastructure.

## What Changes

- Add a unified diff rendering component to the TUI that displays old/new content with red/green line coloring, line numbers, and context lines — reusing the already-defined but unused `diff_added`/`diff_removed`/`diff_added_word`/`diff_removed_word` palette colors.
- Enhance the permission dialog for FileEdit and FileWrite to show a scrollable diff preview below the tool summary, so users can see exactly what will change before approving.
- Integrate the `syntect` library for syntax highlighting in code blocks, supporting at minimum Rust, Python, TypeScript/JavaScript, Go, Java, JSON, YAML, Markdown, Shell, TOML, and C/C++.
- Apply syntax highlighting to both completed-message code blocks and streaming code blocks, with a graceful fallback to monochrome for unrecognized languages.
- Extend the FileEditTool result to include structured diff metadata (path, old_string, new_string, context) so the TUI can render a rich diff view in the tool result area as well.
- Render FileWrite tool use display with file content preview (first N lines) when the content is code.

## Capabilities

### New Capabilities
- `diff-preview`: Unified diff rendering component for the TUI — generates and displays line-level diffs with color coding, line numbers, and context. Used in both permission dialogs and tool result display.
- `syntax-highlighting`: Code block syntax highlighting using syntect — tokenizes source code by language and renders with theme-appropriate colors in ratatui Spans.

### Modified Capabilities
- `tui-markdown-rendering`: Code blocks switch from monochrome `palette.text` rendering to syntax-highlighted output via the new highlighting engine.
- `tui-streaming-markdown`: Streaming code blocks apply syntax highlighting incrementally as lines complete, falling back to plain text during partial-line accumulation.

## Impact

- `crates/tui`: Major changes — new `diff.rs` module (diff computation + rendering), new `highlight.rs` module (syntect integration), modifications to `ui.rs` (code block rendering, permission dialog enhancement, tool result diff display), modifications to `app.rs` (permission dialog state to include diff data, scrollable diff area), modifications to `events.rs` (enhanced ToolUse/PermissionRequest with structured diff info).
- `crates/tui/Cargo.toml`: New dependency on `syntect` for syntax highlighting, `similar` for diff computation.
- `crates/tools/src/file_edit.rs`: Extend `ToolResult` to include structured metadata with old_string/new_string/path for diff rendering.
- `crates/tools/src/file_write.rs`: Extend `ToolResult` to include metadata indicating create vs overwrite and file content preview.
- `crates/tui/src/theme.rs`: No changes needed — diff colors already defined in palette.
- `crates/cli/src/query_loop.rs`: Pass richer tool input data through to TUI bridge for diff preview.
- Performance: syntax highlighting adds per-code-block cost; must stay under 100ms for typical blocks (<500 lines). Diff computation is O(n) and negligible.
