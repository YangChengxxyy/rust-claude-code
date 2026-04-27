## Why

Rust Claude Code currently implements only a subset of the slash commands expected from the upstream Claude Code workflow. Iteration 30 closes several high-frequency command gaps and reduces future command maintenance cost by making command registration self-describing and registry-driven.

## What Changes

- Add TUI slash commands for `/plan`, `/rename`, `/branch`, `/recap`, `/rewind`, `/add-dir`, `/login`, `/logout`, `/effort`, and `/keybindings`.
- Refactor slash command registration from static command tables into a unified dynamic registry with command metadata, dispatch, help output, and suggestion data sourced from the same definitions.
- Add session-level behaviors needed by the new commands, including visible session renaming, conversation branch creation, recap generation, rewind to the previous user turn, additional working directory tracking, and model effort adjustment.
- Add basic Anthropic account command handling that integrates with the existing credential/configuration path without introducing a full OAuth implementation.

## Capabilities

### New Capabilities

### Modified Capabilities

- `slash-command-extensions`: Add the iteration 30 command batch and strengthen the command registry contract so help, suggestions, and dispatch remain consistent.

## Impact

- Affects TUI slash command parsing, suggestion rendering, command dispatch, session state, and help/keybinding output.
- May affect CLI/TUI shared runtime configuration for permission mode, model effort/thinking budget, additional working directories, and credential status messages.
- Adds tests for registry behavior, command availability, session history mutation, and command-specific output.
