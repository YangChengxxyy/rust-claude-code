## 1. Dependencies and Infrastructure

- [x] 1.1 Add `syntect` dependency to `crates/tui/Cargo.toml` (features: default-syntaxes, default-themes, regex-onig or regex-fancy)
- [x] 1.2 Add `similar` dependency to `crates/tui/Cargo.toml` for line-level diff computation
- [x] 1.3 Verify `cargo check -p rust-claude-tui` compiles with new dependencies

## 2. Syntax Highlighting Engine

- [x] 2.1 Create `crates/tui/src/highlight.rs` module with lazy-initialized `SyntaxSet` and `ThemeSet` singletons
- [x] 2.2 Implement `build_custom_theme(palette: &Palette) -> Theme` that maps syntect scopes (keyword, string, comment, type, function, number, operator) to palette-derived colors for both dark and light modes
- [x] 2.3 Implement `highlight_line(line: &str, syntax: &SyntaxReference, state: &mut ParseState, highlighter: &Highlighter) -> Vec<(Style, String)>` that returns styled spans for a single line
- [x] 2.4 Implement `resolve_syntax(language: &str, syntax_set: &SyntaxSet) -> Option<&SyntaxReference>` that handles common language aliases (ts→TypeScript, py→Python, sh→Shell, js→JavaScript, yml→YAML, bash→Bash)
- [x] 2.5 Implement `highlight_code_block(code: &str, language: Option<&str>, palette: &Palette) -> Vec<Vec<(ratatui::style::Style, String)>>` public API that highlights a full code block and returns ratatui-compatible styled spans per line
- [x] 2.6 Write unit tests for highlight module: Rust keywords, Python keywords, unsupported language fallback, no-language fallback, alias resolution
- [x] 2.7 Verify `cargo test -p rust-claude-tui` passes with highlight tests

## 3. Diff Computation and Rendering

- [x] 3.1 Create `crates/tui/src/diff.rs` module
- [x] 3.2 Implement `compute_diff(old: &str, new: &str) -> Vec<DiffLine>` using `similar::TextDiff` that returns a list of `DiffLine { kind: Added|Removed|Context, content: String, old_lineno: Option<usize>, new_lineno: Option<usize> }`
- [x] 3.3 Implement `render_diff_lines(diff_lines: &[DiffLine], palette: &Palette, width: u16) -> Vec<Line>` that renders diff lines as ratatui `Line` objects with `+`/`-` markers, line numbers, and palette diff colors
- [x] 3.4 Implement `render_file_preview(content: &str, palette: &Palette, max_lines: usize, width: u16) -> Vec<Line>` for FileWrite new-file preview (all lines as additions, truncated at max_lines with indicator)
- [x] 3.5 Write unit tests for diff module: single-line replacement, multi-line addition, empty old_string, large diff truncation, line number correctness
- [x] 3.6 Verify `cargo test -p rust-claude-tui` passes with diff tests

## 4. Permission Dialog Enhancement

- [x] 4.1 Extend `PermissionDialog` struct in `app.rs` to include `diff_lines: Option<Vec<DiffLine>>`, `diff_scroll: usize`, `is_file_tool: bool`, and `file_path: Option<String>`
- [x] 4.2 Update `App::show_permission_dialog()` to extract `old_string`/`new_string` from FileEdit input and `content`/`file_path` from FileWrite input, compute diff lines, and populate the new fields
- [x] 4.3 Rewrite `draw_permission_dialog()` in `ui.rs` to use dynamic sizing: 80% terminal width (max 120), 80% terminal height (min 16), with header section (tool info + options) and scrollable diff area
- [x] 4.4 Implement diff area scrolling: Up/Down arrows scroll the diff view when the permission dialog is active (override normal navigation)
- [x] 4.5 Add `replace_all` indicator in the dialog header when FileEdit has `replace_all: true`
- [x] 4.6 For non-file tools (Bash, etc.), retain the compact dialog layout (original 50x12 box)
- [x] 4.7 Write unit tests for permission dialog state: diff extraction from FileEdit input, diff extraction from FileWrite input, scroll bounds, non-file-tool fallback
- [x] 4.8 Verify `cargo test -p rust-claude-tui` passes with dialog tests

## 5. Code Block Syntax Highlighting Integration

- [x] 5.1 Update `render_markdown_message()` in `ui.rs` to call `highlight_code_block()` for code blocks with a language tag, replacing the current monochrome `Span::styled(line, palette.text)` rendering
- [x] 5.2 Update `render_markdown_message()` to fall back to plain `palette.text` rendering when language is None or unrecognized (matching current behavior)
- [x] 5.3 Update `StreamingMarkdownState` in `streaming_markdown.rs` to maintain a `ParseState` per code block and call `highlight_line()` for each completed line inside a code block
- [x] 5.4 Ensure streaming pending line (incomplete, no trailing newline) renders in plain `palette.text` without highlighting
- [x] 5.5 Verify syntax highlighting works in both dark and light palette modes
- [x] 5.6 Write integration test: render a message containing a Rust code block and verify highlighted spans have non-default colors
- [x] 5.7 Verify `cargo test -p rust-claude-tui` passes

## 6. Tool Result Diff Display

- [x] 6.1 Extend `ChatMessage::ToolUse` in `events.rs` to include `diff_lines: Option<Vec<DiffLine>>` for FileEdit/FileWrite tools
- [x] 6.2 Update the tool use message creation path in `app.rs` to populate `diff_lines` from the stored tool input when the tool is FileEdit or FileWrite
- [x] 6.3 Update `render_tool_use_message()` or equivalent in `ui.rs` to render a compact diff view (max 20 lines, truncated with indicator) below the tool name for messages with `diff_lines`
- [x] 6.4 Write unit tests for tool result diff display: small diff fully shown, large diff truncated
- [x] 6.5 Verify `cargo test -p rust-claude-tui` passes

## 7. Final Integration and Verification

- [x] 7.1 Run `cargo check --workspace` to verify no compilation errors across all crates
- [x] 7.2 Run `cargo test --workspace` to verify all existing and new tests pass (108 TUI + 336 others = 444 total; CLI pre-existing test issue excluded)
- [x] 7.3 Run `cargo clippy --workspace` and fix any new warnings (removed unused import, prefixed unused param)
- [ ] 7.4 Manual test: start TUI, trigger a FileEdit via the model, verify diff preview appears in the permission dialog with colored +/- lines
- [ ] 7.5 Manual test: verify code blocks in assistant responses show syntax highlighting for Rust and Python
- [ ] 7.6 Manual test: verify streaming code blocks show highlighting incrementally as lines complete
- [ ] 7.7 Manual test: verify non-file-tool (Bash) permission dialog retains compact layout
- [ ] 7.8 Manual test: verify light theme shows readable diff and highlight colors
