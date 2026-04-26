## Context

Iteration 23 delivered streaming markdown rendering, thinking streaming, and tool input streaming. The TUI can now display formatted text in real time. However, two critical gaps remain:

1. **No diff preview**: The permission dialog for FileEdit/FileWrite shows only `"src/main.rs (edit)"` — users approve or deny changes blindly. The `PermissionDialog` struct holds `tool_name` and `input_summary` (a one-line string) with no structured data about the actual change.

2. **No syntax highlighting**: Code blocks render all lines in `palette.text` (monochrome). The `render_markdown_message` function in `ui.rs` uses a single `Span::styled(line, palette.text)` for every code line. The streaming markdown module (`streaming_markdown.rs`) has the same limitation.

Current state of relevant code:
- `crates/tools/src/file_edit.rs`: Returns `ToolResult::success(id, "Edited {path}")` — no structured diff data.
- `crates/tui/src/app.rs`: `PermissionDialog` has `tool_name`, `input_summary`, `selected`, `response_tx`. The `summarize_tool_input` function returns `"{path} (edit)"` for FileEdit.
- `crates/tui/src/ui.rs`: `draw_permission_dialog` renders a 50x12 centered modal. Code blocks use `Span::styled(line, palette.text)`.
- `crates/tui/src/theme.rs`: Palette already includes `diff_added`, `diff_removed`, `diff_added_word`, `diff_removed_word` colors (unused).
- `crates/tui/Cargo.toml`: No syntax highlighting or diff libraries.

## Goals / Non-Goals

**Goals:**

- Users can see a colored unified diff in the permission dialog before approving FileEdit/FileWrite operations.
- Code blocks in assistant messages and tool results display language-aware syntax highlighting for common languages.
- Streaming code blocks apply syntax highlighting incrementally as complete lines arrive.
- The diff and highlight infrastructure is reusable across permission dialogs, tool result display, and markdown code blocks.
- Performance stays within acceptable bounds: <100ms per code block highlight, <50ms per streaming frame.

**Non-Goals:**

- Tree-sitter integration or structural language analysis — syntect is sufficient for color-based highlighting.
- Custom theme files for syntax highlighting — use a single embedded theme (e.g., base16-ocean or inspired-github) that maps well to our dark/light palette.
- Inline diff (word-level diff within changed lines) — line-level diff is sufficient for this iteration. Word-level highlighting can be a follow-up.
- Diff preview for BashTool or other non-file tools.
- FileEditTool returning the actual file content diff (before/after) — we compute the diff from old_string/new_string already present in the tool input.

## Decisions

### D1: Use `syntect` for syntax highlighting

**Choice**: `syntect` crate (Rust port of Sublime Text syntax definitions).

**Alternatives considered**:
- `tree-sitter-highlight`: More accurate parsing but requires per-language grammar binaries (~2MB each), complex build setup, and pulls in C dependencies. Overkill for syntax coloring in a TUI.
- Custom keyword-based coloring: Simple but brittle, language-specific, and impossible to maintain for 10+ languages.
- `bat`'s highlighting module: Wraps syntect anyway, adds unnecessary dependencies.

**Rationale**: syntect is pure Rust, has a small binary footprint when using embedded default syntaxes, supports 100+ languages out of the box, and maps highlighted tokens to ratatui `Style` naturally. It is the standard choice for Rust TUI syntax highlighting.

**Key details**:
- Use `syntect::parsing::SyntaxSet::load_defaults_newlines()` for built-in syntaxes.
- Use `syntect::highlighting::ThemeSet::load_defaults()` and select a theme compatible with dark/light palettes, or build a custom `Theme` that maps to our `Palette` colors.
- Highlight on a per-line basis so it integrates with both the completed-message renderer and the streaming renderer.
- Lazy-initialize `SyntaxSet` and `ThemeSet` as static singletons (via `once_cell::sync::Lazy` or `std::sync::LazyLock`) — these are expensive to construct.

### D2: Use `similar` for diff computation

**Choice**: `similar` crate (Rust diff library, based on Myers diff algorithm).

**Alternatives considered**:
- `diff` crate: Older, less maintained, API is less ergonomic.
- `diffy` crate: Wraps `similar`, adds unified diff text formatting we don't need (we render our own).
- Manual implementation: Unnecessary given well-tested libraries exist.

**Rationale**: `similar` is actively maintained, has a clean API (`TextDiff::from_lines()`), supports line-level and word-level diffs, and is used by major Rust projects (insta, cargo).

### D3: Compute diff from tool input, not tool output

**Choice**: Generate the diff preview in the TUI from the `old_string` and `new_string` fields already present in the `PermissionRequest` event's `input` JSON. Do NOT modify FileEditTool's return value.

**Alternatives considered**:
- Modify `FileEditTool.execute()` to return structured diff metadata in `ToolResult`: Requires changing the tool trait contract, adds complexity to tool_types, and the information is already available in the input.
- Read the file in the TUI to compute full-file diff: Breaks TUI's role as a pure rendering layer; introduces file I/O in the display thread.

**Rationale**: The permission dialog already receives the full `input: serde_json::Value` in the `PermissionRequest` event. For FileEdit, this contains `path`, `old_string`, and `new_string`. For FileWrite, it contains `path` and `content`. We can compute and render the diff entirely within the TUI layer using these fields. This avoids tool trait changes and keeps the TUI self-contained.

For the tool result display (after execution), we similarly extract diff info from the `ToolUseStart` input that was already stored in `ChatMessage::ToolUse`.

### D4: Extend PermissionDialog to include scrollable diff area

**Choice**: Expand the permission dialog to use most of the terminal height. The dialog will have two sections: a fixed-height header (tool name, summary, options) and a scrollable diff preview area below it.

**Key details**:
- Dialog width: 80% of terminal width (capped at 120 cols).
- Dialog height: 80% of terminal height (min 16 rows).
- Top section (fixed, ~8 rows): tool name, file path, 4 option buttons.
- Bottom section (remaining height): scrollable diff view with line numbers, `+`/`-` markers, color coding.
- Scroll: Up/Down arrows scroll the diff when dialog is open (overrides normal navigation).
- For FileWrite (new file creation): show the first N lines of content as a "preview" instead of a diff.

### D5: Syntax highlight theme mapping strategy

**Choice**: Create a custom `syntect::highlighting::Theme` at runtime that maps highlight scopes to colors derived from our `Palette`. This ensures syntax colors are consistent with the current dark/light theme.

**Alternative considered**:
- Use a built-in syntect theme (e.g., `base16-ocean.dark`): Simple but colors may clash with our palette, especially in light mode.

**Rationale**: Building a small custom theme (~20 scope mappings for keywords, strings, comments, types, operators, numbers, functions) ensures visual coherence. The mapping is straightforward: `keyword` → a brighter shade of `palette.claude`, `string` → `palette.success` tint, `comment` → `palette.inactive`, etc.

### D6: Highlight integration in streaming markdown

**Choice**: Apply syntax highlighting to completed lines only. The pending (incomplete) line renders with plain `palette.text`.

**Rationale**: syntect line highlighting requires knowing the state from the previous line. We maintain a `HighlightState` per code block that advances as each complete line is added. This is efficient (no re-highlighting) and avoids artifacts from highlighting partial lines. Since streaming typically delivers lines faster than humans read, the momentary plain rendering of the pending line is imperceptible.

## Risks / Trade-offs

- **[Binary size increase from syntect]** → syntect's default syntax set adds ~2-4MB to the binary (compressed syntax definitions). Mitigation: acceptable for a CLI tool; can slim down in the future by selecting only used syntaxes with `syntect::dumps::from_binary`.

- **[Highlighting performance on very large code blocks]** → A 1000-line code block could take >100ms to highlight in one shot. Mitigation: highlight lazily per-line as rendered, cache results. In streaming mode this is naturally incremental.

- **[Permission dialog complexity increase]** → The dialog goes from a simple 50x12 box to a scrollable multi-section component. Mitigation: keep the interaction model simple — same 4 options with same keyboard shortcuts, just more visual information. If the diff is empty or too short, collapse to the original compact dialog.

- **[syntect theme not perfectly matching terminal palette]** → Custom theme colors may look slightly off on terminals with non-standard color rendering. Mitigation: use only the 8 base colors from our Palette (which are already tested) and derive highlight colors from them.
