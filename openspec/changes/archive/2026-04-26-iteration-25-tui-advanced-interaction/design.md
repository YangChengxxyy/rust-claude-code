## Context

The TUI already has a unified slash command table in `crates/tui/src/app.rs`, a `UserCommand` channel from TUI to the CLI worker, and a basic theme path with `Theme::Dark` / `Theme::Light`. Sessions are currently stored as full JSON files under `~/.config/rust-claude-code/sessions/`, and resume support exists for CLI flags (`--resume <id>`, `--continue`) but not for an interactive TUI picker.

Iteration 25 builds on that foundation rather than introducing a separate interaction framework. The main cross-cutting concern is that session listing, export, context accounting, clipboard access, and theme persistence all require the TUI to request data or side effects from the CLI worker without blocking rendering.

## Goals / Non-Goals

**Goals:**

- Provide an interactive recent-session picker for `/resume` in the TUI.
- Make context-window usage visible through `/context`.
- Support Markdown transcript export through `/export`.
- Support copying the latest assistant response through `/copy`.
- Complete the theme system with built-in dark/light themes plus a custom theme file and immediate repaint.
- Keep long-running filesystem, clipboard, and session operations off the TUI event loop.

**Non-Goals:**

- Remote session synchronization or cloud-backed session search.
- Full terminal mouse-driven picker interactions; keyboard support is sufficient.
- Arbitrary transcript export formats beyond Markdown.
- Per-language syntax theme customization; custom themes only affect the TUI palette.
- Exact tokenization of every context component. Iteration 25 can use known usage totals and conservative estimates where exact API tokenizer data is unavailable.

## Decisions

### D1: Add a session summary type and scan session files lazily

**Choice**: Add `SessionSummary` beside `SessionFile` in `crates/cli/src/session.rs`, with `id`, `model`, `model_setting`, `cwd`, `created_at`, `updated_at`, `message_count`, `first_user_summary`, and optional `total_usage`.

**Alternatives considered**:
- Load full sessions directly in the TUI. This couples rendering code to persistence and can block on large files.
- Maintain a separate index file. Faster long term, but adds migration and consistency complexity before the session list is large enough to need it.

**Rationale**: A lightweight summary keeps the picker responsive and preserves the current JSON session format. The CLI worker can scan the sessions directory on demand with `spawn_blocking`, sort by `updated_at` or timestamp filename descending, and cap to a default of 20 recent sessions.

### D2: Model the session picker as TUI state, not as a slash-command text response

**Choice**: Add a `SessionPicker` modal state in the TUI with summaries, selected index, scroll offset, loading/error states, and keyboard handling for Up/Down/PageUp/PageDown/Enter/Escape.

**Alternatives considered**:
- Print a numbered list and require `/resume <number>`. Simpler, but slower and easier to mis-select.
- Reuse permission dialog state. The picker has different keyboard behavior, pagination, and data shape.

**Rationale**: A modal picker matches the rest of the TUI interaction model and allows selection without mutating the input buffer. The TUI asks the worker for summaries through `UserCommand::ListSessions`, receives them through a new `AppEvent::SessionList`, then sends `UserCommand::ResumeSession(id)` on selection.

### D3: Restore selected sessions through the existing CLI restore path

**Choice**: Extract the current `restore_session` closure in `crates/cli/src/main.rs` into a reusable helper and use it for CLI flags and TUI `ResumeSession`.

**Alternatives considered**:
- Duplicate restore logic inside the TUI worker branch. This risks drift across model settings, permissions, usage, and history restoration.

**Rationale**: Resume behavior must remain identical whether invoked by `--resume`, `--continue`, or the interactive picker. The worker should load the selected `SessionFile`, replace the shared `AppState` messages/settings/usage/rules, and emit events that refresh the visible transcript and status bar.

### D4: Implement `/context` from a structured context snapshot

**Choice**: Add a `ContextSnapshot` value produced by the CLI worker from current `AppState`: estimated max context, system prompt estimate, message estimate, tool result estimate, remaining estimate, and the latest reported usage when available.

**Alternatives considered**:
- Render only raw token usage from `/cost`. That does not explain what is consuming the context window.
- Require exact tokenizer support before showing the command. This would delay a useful approximation.

**Rationale**: Users need actionable proportions, not perfect token accounting. The output can render a compact colored bar in the TUI plus numeric token counts and percentages. If model context capacity is unknown, the command should say so and still show known usage totals.

### D5: Keep export and clipboard side effects in the CLI worker

**Choice**: Add `UserCommand::ExportConversation { path: Option<PathBuf> }` and `UserCommand::CopyLatestAssistant`. The worker performs filesystem and clipboard operations via `spawn_blocking` and returns success/error text to the TUI.

**Alternatives considered**:
- Perform export/copy directly inside `App::handle_slash_command`. This blocks input/rendering and makes TUI tests depend on host clipboard/file behavior.

**Rationale**: The worker already owns `AppState`, the full conversation, and side-effectful command handling. TUI command handling remains a dispatcher plus immediate feedback. Markdown export should include session metadata, user/assistant messages, thinking blocks when available, and tool use/result sections in fenced code blocks.

### D6: Extend the existing theme model with a custom palette variant

**Choice**: Add a TUI-local custom theme loader that reads `~/.config/rust-claude-code/theme.json` into a serializable palette override and applies it to `Palette`. Keep `ConfigTheme` as the built-in persisted enum for dark/light, and store custom selection as TUI-specific config if widening the core config enum would disrupt existing config compatibility.

**Alternatives considered**:
- Replace the core `Theme` enum with a fully dynamic theme type immediately. This is more invasive and risks config migration problems.
- Only support dark/light. That fails the iteration 25 requirement for custom theme files.

**Rationale**: The renderer only needs a `Palette`. Loading custom colors at TUI startup and on `/theme custom` keeps the design local while preserving current config behavior. Invalid custom files should produce a clear error and leave the active theme unchanged.

### D7: Use one slash command registry as the command source of truth

**Choice**: Extend the existing `SLASH_COMMANDS` table to include `/resume`, `/context`, `/export`, and `/copy`, and update `/theme` usage to include list/custom forms.

**Alternatives considered**:
- Handle hidden commands outside the registry. This creates help/dispatch drift.

**Rationale**: Iteration 17 already established registry-based command help. New commands must participate in the same path so validation, help text, and tests stay aligned.

## Risks / Trade-offs

- **[Large session directories can make `/resume` slow]** -> Scan in `spawn_blocking`, cap the default list to recent 20 sessions, and tolerate unreadable/corrupt files by reporting skipped entries rather than failing the whole picker.
- **[Context accounting is approximate]** -> Label estimates clearly, prefer latest API usage when available, and keep the breakdown stable enough to compare across turns.
- **[Clipboard support varies by platform and terminal environment]** -> Use a maintained cross-platform dependency with clear errors when the clipboard provider is unavailable.
- **[Custom theme files can make text unreadable]** -> Validate required color fields and fall back to the current theme when parsing fails. Tests should cover invalid JSON and missing fields.
- **[Resuming from TUI can conflict with an active stream]** -> Disable resume selection while a stream is active or require cancellation first.
