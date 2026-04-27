## 1. Registry Refactor

- [x] 1.1 Add a slash command registry type that stores command name, usage, description, and dispatch metadata for built-in commands.
- [x] 1.2 Move existing command metadata from the static `SLASH_COMMANDS` slice into registry initialization without changing existing command names or usage text.
- [x] 1.3 Update `/help`, command validation, and slash suggestion generation to read command definitions from the registry.
- [x] 1.4 Add registry tests proving help output, suggestions, and command validation are derived from the same command definitions.

## 2. Command Routing

- [x] 2.1 Add `UserCommand` variants for `/plan`, `/rename`, `/branch`, `/recap`, `/rewind`, `/add-dir`, `/login`, `/logout`, `/effort`, and `/keybindings` as needed for worker-handled behavior.
- [x] 2.2 Register all iteration 30 commands with usage strings and descriptions so they appear in `/help` and slash suggestions.
- [x] 2.3 Update TUI slash command parsing to validate arguments and route each new command to local output or typed worker commands.
- [x] 2.4 Add TUI command parsing tests for valid and invalid arguments for each new command.

## 3. Session Commands

- [x] 3.1 Implement `/plan [description]` by switching to plan permission mode and optionally routing description text as planning context.
- [x] 3.2 Implement `/rename <name>` so the visible session name and session metadata are updated or usage guidance is shown for missing names.
- [x] 3.3 Implement `/branch [name]` so current conversation history is forked into an independent active branch with a displayed explicit or generated name.
- [x] 3.4 Implement `/recap` so the current conversation summary is displayed, with a safe empty-conversation message.
- [x] 3.5 Implement `/rewind` so the latest user turn and following assistant/thinking/tool outputs are removed from conversation context and visible TUI state.
- [x] 3.6 Add tests for session rename, branch creation, recap output, and rewind behavior.

## 4. Workspace, Account, And Model Commands

- [x] 4.1 Implement `/add-dir <path>` with path canonicalization, existing-directory validation, duplicate handling, and confirmation output.
- [x] 4.2 Implement `/login` to report the active credential source or setup guidance using existing config, environment, Claude settings, and `apiKeyHelper` resolution paths.
- [x] 4.3 Implement `/logout` to clear only safe local rust-claude config credentials and explain external credential sources that must be changed outside the TUI.
- [x] 4.4 Implement `/effort [low|medium|high]` with current-value display, validation, runtime state update, and request construction integration for thinking-budget-aware models.
- [x] 4.5 Implement `/keybindings` to display active shortcuts for editing, navigation, streaming control, dialogs, and thinking blocks.
- [x] 4.6 Add tests for add-dir validation, login/logout output paths, effort validation/state changes, and keybindings output.

## 5. Verification

- [x] 5.1 Run `cargo test -p rust-claude-tui` and fix any TUI regressions.
- [x] 5.2 Run `cargo test -p rust-claude-cli` and fix any worker/session command regressions.
- [x] 5.3 Run `cargo test --workspace` and fix any workspace regressions.
- [ ] 5.4 Manually smoke-test the new commands in the TUI enough to confirm `/help`, suggestions, and command output are coherent.
