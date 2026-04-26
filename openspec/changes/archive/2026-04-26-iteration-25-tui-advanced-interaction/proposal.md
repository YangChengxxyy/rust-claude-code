## Why

Iteration 23 made the TUI feel alive through streaming output, and iteration 24 made it safer to review code through diff preview and syntax highlighting. The next daily-use gap is interaction depth: users still lack a first-class session picker, context visibility, export/copy affordances, and runtime theme switching inside the TUI.

## What Changes

- Add an interactive `/resume` session picker that lists recent sessions with timestamp, model, message count, and first-user-message summary, then restores the selected session with keyboard navigation.
- Add a `/context` command that renders a readable context-window usage visualization split by system prompt, messages, tool results, and remaining capacity.
- Add a `/export` command that writes the current conversation transcript to a Markdown file with metadata and readable message/tool sections.
- Add a `/copy` command that copies the latest assistant response to the system clipboard and reports clear success or failure.
- Add a runtime theme system with built-in `dark` and `light` themes plus an optional custom theme file at `~/.config/rust-claude-code/theme.json`.
- Add a `/theme` command for listing and switching themes, with immediate TUI repaint after a theme change.
- Extend session metadata storage/querying so the CLI/TUI can efficiently list recent sessions without loading every full transcript.

## Capabilities

### New Capabilities
- `tui-session-picker`: Interactive recent-session selection UI and supporting session metadata model.
- `tui-context-visualization`: Context-window usage accounting and visual display in the TUI.
- `conversation-export`: Markdown export of the current conversation transcript.
- `clipboard-copy`: Copying the latest assistant response to the system clipboard from the TUI.
- `tui-theme-system`: Built-in and custom TUI themes with runtime switching.

### Modified Capabilities
- `session-resume`: `/resume` gains an interactive picker path in addition to explicit session id resume behavior.
- `slash-command-extensions`: Register `/context`, `/export`, `/copy`, and `/theme` through the unified slash command registry and include them in help output.

## Impact

- `crates/tui`: Major changes to app state, event handling, session picker rendering, context/export/copy/theme command output, and theme loading/application.
- `crates/cli`: Add session-list query support and route enhanced session metadata to the TUI; wire the TUI `/resume` flow into existing session loading.
- `crates/core`: Extend session metadata types to include first-message summary, message count, model, timestamps, and usage/context accounting data where appropriate.
- `Cargo.toml` / crate manifests: Likely add a small clipboard dependency (for example `arboard` or `copypasta`) and JSON theme deserialization support where not already available.
- Local files: Read optional theme configuration from `~/.config/rust-claude-code/theme.json`; write Markdown exports to a user-visible path chosen by command argument or default export directory.
