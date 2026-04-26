## Why

Iteration 23 through 25 focused on the TUI experience: streaming, diff review, syntax highlighting, session picking, context visibility, export, copy, and themes. The next daily-use ceiling is agent capability. Three gaps still make the Rust implementation feel less capable than the reference workflow:

- Bash does not preserve shell working directory changes, so `cd some/dir` has no effect on later commands.
- The model cannot ask the user a structured follow-up question from inside the tool loop.
- `WebSearchTool` has the right interface but still uses a dummy backend, so it cannot retrieve real search results.

Iteration 26 closes those gaps while preserving the existing tool, permission, TUI bridge, and settings patterns.

## What Changes

- Make Bash command execution use a session-persistent current working directory.
- Update the session CWD after Bash commands that change directories, using normal shell semantics rather than project-root confinement in this iteration.
- Add `AskUserQuestionTool`, allowing the model to present a question with labeled options and optional custom text.
- Add TUI support for rendering AskUserQuestion prompts as an interactive modal and returning the selected/custom answer through a oneshot response channel.
- Add print/non-interactive behavior for AskUserQuestion, choosing a deterministic fallback when no interactive UI is available.
- Replace the WebSearch dummy backend with at least one real configurable backend, preserving the existing `query`, `allowed_domains`, and `blocked_domains` tool interface.
- Add settings/env configuration for WebSearch provider selection and credentials.

## Capabilities

### New Capabilities

- `bash-persistent-cwd`: Bash commands start from the current session CWD and may update it for subsequent Bash commands.
- `ask-user-question-tool`: A structured user-question tool with TUI modal interaction and non-interactive fallback behavior.

### Modified Capabilities

- `web-search`: `WebSearchTool` uses a real configured provider instead of the dummy backend, while retaining structured result formatting and domain filters.

## Impact

- `crates/core`: May extend `AppState` helpers around `cwd` and add configuration fields for WebSearch provider/API key if those fields belong in core config.
- `crates/tools`: Update `BashTool`, add `AskUserQuestionTool`, extend `ToolContext` with user-question plumbing, and implement a real `SearchBackend`.
- `crates/cli`: Wire AskUserQuestion callbacks into `QueryLoop`/TUI bridge, register the new tool, pass WebSearch configuration into tool construction, and define non-interactive fallback behavior.
- `crates/tui`: Add an AskUserQuestion modal state, rendering, keyboard handling, and response event path.
- `settings.json` / env: Support WebSearch provider and credential configuration. Environment variables should remain usable for secrets.

