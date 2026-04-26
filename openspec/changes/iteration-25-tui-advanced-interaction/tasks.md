## 1. Session Metadata and Resume Plumbing

- [x] 1.1 Add `SessionSummary` and helper methods in `crates/cli/src/session.rs` to derive id, timestamps, model, cwd, message count, first user summary, and usage from a `SessionFile`
- [x] 1.2 Implement `list_recent_sessions(limit: usize) -> Result<Vec<SessionSummary>>` that scans session JSON files newest-first, skips corrupt files, and caps the result set
- [x] 1.3 Extract the existing CLI session restore closure into a reusable helper that restores messages, usage, model settings, permission mode, and allow/deny rules
- [x] 1.4 Add tests for session summary extraction, recent-session sorting, corrupt-file skipping, and direct resume helper behavior

## 2. TUI Command and Event Model

- [x] 2.1 Extend `UserCommand` with `ListSessions`, `ResumeSession(String)`, `ShowContext`, `ExportConversation { path: Option<PathBuf> }`, and `CopyLatestAssistant`
- [x] 2.2 Extend `AppEvent` with session-list/session-resumed events and a structured context display event or equivalent message path
- [x] 2.3 Update the slash command registry to include `/resume [session-id]`, `/context`, `/export [path]`, `/copy`, and `/theme [dark|light|custom]`
- [x] 2.4 Update slash command dispatch to send the new `UserCommand` variants and preserve unknown-command behavior
- [x] 2.5 Add TUI unit tests for command registry help output, command validation, argument parsing, and dispatch variants

## 3. Interactive Session Picker

- [x] 3.1 Add `SessionPicker` state to `crates/tui/src/app.rs` with loading, error, summaries, selected index, and scroll offset
- [x] 3.2 Render the session picker modal with recent session rows showing id, updated time, model, message count, cwd, and first user summary
- [x] 3.3 Implement keyboard handling for Up, Down, PageUp, PageDown, Enter, and Escape while the picker is active
- [x] 3.4 Wire `/resume` with no argument to request recent sessions and open the picker; wire `/resume <id>` to resume directly
- [x] 3.5 In the CLI worker, handle `ListSessions` and `ResumeSession` via `spawn_blocking` where file I/O is involved
- [x] 3.6 Add tests for empty session list, picker navigation bounds, cancel behavior, selected-session resume command, deleted-session error, and active-stream blocking

## 4. Context Visualization

- [x] 4.1 Add a `ContextSnapshot` data type containing model, known capacity, used tokens, system prompt estimate, message estimate, tool result estimate, and remaining estimate
- [x] 4.2 Implement context snapshot construction from `AppState`, preferring recorded usage totals and using conservative estimates for component breakdowns
- [x] 4.3 Render `/context` output as a segmented usage bar when terminal width allows and aligned text rows in narrow terminals
- [x] 4.4 Handle unknown model context capacity with a clear unavailable-capacity message while still showing known usage
- [x] 4.5 Add tests for known capacity, unknown capacity, tool-result segment accounting, post-turn usage updates, and narrow-width fallback formatting

## 5. Markdown Export

- [x] 5.1 Implement Markdown transcript formatting for session metadata, user messages, assistant messages, thinking blocks, tool uses, and tool results
- [x] 5.2 Add default export path generation based on session id or timestamp when `/export` has no explicit path
- [x] 5.3 Handle explicit `/export <path>` paths, parent directory creation, write errors, and success reporting
- [x] 5.4 Run export file I/O in the CLI worker off the TUI event loop
- [x] 5.5 Add tests for explicit path export, default path export, metadata inclusion, chronological message ordering, tool block formatting, and write failure reporting

## 6. Clipboard Copy

- [x] 6.1 Add a cross-platform clipboard dependency and wrapper function that copies text with clear error reporting
- [x] 6.2 Implement latest completed assistant response extraction from the current conversation while ignoring active streaming text
- [x] 6.3 Handle `/copy` through the CLI worker and show success, no-assistant-message, streaming, and clipboard-unavailable messages
- [x] 6.4 Add tests for latest assistant selection, no-response behavior, active-stream behavior, and clipboard wrapper error mapping

## 7. Theme System

- [x] 7.1 Define a serializable custom theme file format for `~/.config/rust-claude-code/theme.json` with all required `Palette` colors
- [x] 7.2 Implement custom theme loading, color parsing, validation, and conversion into `Palette`
- [x] 7.3 Update `/theme` with no argument to list active and available themes; support `/theme dark`, `/theme light`, and `/theme custom`
- [x] 7.4 Ensure dark/light selections persist through existing config saving and custom theme parse failures leave the active theme unchanged
- [x] 7.5 Add tests for dark/light switching, custom theme success, missing custom file, invalid JSON, invalid color values, and help text

## 8. Integration and Verification

- [x] 8.1 Run `cargo fmt --all`
- [x] 8.2 Run `cargo test -p rust-claude-cli`
- [x] 8.3 Run `cargo test -p rust-claude-tui`
- [x] 8.4 Run `cargo test --workspace`
- [ ] 8.5 Manual test: open the TUI, run `/resume`, navigate the picker, and restore a previous session
- [ ] 8.6 Manual test: run `/context`, `/export`, `/copy`, and `/theme custom` in the TUI and verify user-facing output
