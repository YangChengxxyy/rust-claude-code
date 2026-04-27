## ADDED Requirements

### Requirement: Built-in slash commands SHALL include iteration 30 command batch
The built-in slash command set SHALL include `/plan`, `/rename`, `/branch`, `/recap`, `/rewind`, `/add-dir`, `/login`, `/logout`, `/effort`, and `/keybindings` as first-party commands available from the TUI.

#### Scenario: Help output lists iteration 30 commands
- **WHEN** the user runs `/help`
- **THEN** the help output SHALL include `/plan`, `/rename`, `/branch`, `/recap`, `/rewind`, `/add-dir`, `/login`, `/logout`, `/effort`, and `/keybindings` with brief descriptions

#### Scenario: Command validation recognizes iteration 30 commands
- **WHEN** slash command parsing validates one of `/plan`, `/rename`, `/branch`, `/recap`, `/rewind`, `/add-dir`, `/login`, `/logout`, `/effort`, or `/keybindings`
- **THEN** the command SHALL be accepted as a supported built-in command

### Requirement: Plan command enters plan mode with optional context
The TUI SHALL support `/plan [description]` to switch the session to plan permission mode and optionally submit the provided description as planning context.

#### Scenario: Plan command without description
- **WHEN** the user runs `/plan` without arguments
- **THEN** the session SHALL switch to plan mode and display confirmation that plan mode is active

#### Scenario: Plan command with description
- **WHEN** the user runs `/plan investigate streaming bugs`
- **THEN** the session SHALL switch to plan mode and route `investigate streaming bugs` as the next planning prompt or planning context

### Requirement: Rename command updates visible session name
The TUI SHALL support `/rename <name>` to rename the current session for display and subsequent session metadata updates.

#### Scenario: Rename current session
- **WHEN** the user runs `/rename frontend cleanup`
- **THEN** the current session display name SHALL become `frontend cleanup`

#### Scenario: Rename without name is rejected
- **WHEN** the user runs `/rename` without a name
- **THEN** the TUI SHALL show usage guidance and leave the session name unchanged

### Requirement: Branch command forks conversation history
The TUI SHALL support `/branch [name]` to create an independent conversation branch from the current message history.

#### Scenario: Branch current conversation
- **WHEN** the user runs `/branch experiment-a`
- **THEN** a new branch SHALL be created with the current conversation history and the active session SHALL continue on that independent branch

#### Scenario: Branch without explicit name
- **WHEN** the user runs `/branch` without a name
- **THEN** a branch name SHALL be generated and displayed to the user

### Requirement: Recap command summarizes current session
The TUI SHALL support `/recap` to generate or display a concise summary of the current conversation state.

#### Scenario: Recap with existing conversation
- **WHEN** the current session contains user or assistant messages and the user runs `/recap`
- **THEN** the TUI SHALL display a concise summary of the current session

#### Scenario: Recap with empty conversation
- **WHEN** the user runs `/recap` before any conversation messages exist
- **THEN** the TUI SHALL report that there is no conversation to summarize without failing

### Requirement: Rewind command removes the latest user turn
The TUI SHALL support `/rewind` to roll back conversation state to before the most recent user message and its corresponding assistant/tool outputs.

#### Scenario: Rewind after completed assistant turn
- **WHEN** the session contains at least one completed user turn and the user runs `/rewind`
- **THEN** the latest user message and following assistant, thinking, tool, and system outputs belonging to that turn SHALL be removed from conversation context

#### Scenario: Rewind with no user turn
- **WHEN** the user runs `/rewind` before any user turn exists
- **THEN** the TUI SHALL report that there is no user turn to rewind

### Requirement: Add-dir command registers additional working directories
The TUI SHALL support `/add-dir <path>` to add an existing directory to the current session's workspace context.

#### Scenario: Add existing directory
- **WHEN** the user runs `/add-dir ../shared` and the path resolves to an existing directory
- **THEN** the resolved directory SHALL be added to the session workspace context and shown in confirmation output

#### Scenario: Add missing directory is rejected
- **WHEN** the user runs `/add-dir /path/that/does/not/exist`
- **THEN** the TUI SHALL show an error and leave the workspace directory list unchanged

### Requirement: Login and logout commands manage credential status at a basic level
The TUI SHALL support `/login` and `/logout` for basic Anthropic credential status, setup guidance, and safe local credential cleanup.

#### Scenario: Login reports configured authentication path
- **WHEN** the user runs `/login`
- **THEN** the TUI SHALL report whether credentials are available from rust-claude config, environment variables, Claude settings, or `apiKeyHelper`, and SHALL show setup guidance when none are available

#### Scenario: Logout clears safe local credentials or explains external sources
- **WHEN** the user runs `/logout`
- **THEN** the TUI SHALL clear credentials only from local rust-claude configuration when possible, or explain that environment variables or Claude settings must be changed outside the TUI

### Requirement: Effort command controls model effort level
The TUI SHALL support `/effort [low|medium|high]` to inspect or set the current model effort level used for thinking-budget-aware requests.

#### Scenario: Set effort level
- **WHEN** the user runs `/effort high`
- **THEN** subsequent model requests SHALL use the high effort mapping and the TUI SHALL display confirmation

#### Scenario: Show current effort level
- **WHEN** the user runs `/effort` without arguments
- **THEN** the TUI SHALL display the current effort level and valid values

#### Scenario: Invalid effort level is rejected
- **WHEN** the user runs `/effort extreme`
- **THEN** the TUI SHALL show valid values and leave the current effort level unchanged

### Requirement: Keybindings command displays keyboard shortcuts
The TUI SHALL support `/keybindings` to display the active keyboard shortcuts for editing, navigation, streaming control, dialogs, and thinking blocks.

#### Scenario: Show keybindings
- **WHEN** the user runs `/keybindings`
- **THEN** the TUI SHALL display the active keybindings, including submit, multiline input, history navigation, chat scrolling, cancel stream, redraw, and thinking toggle shortcuts

## MODIFIED Requirements

### Requirement: Slash commands are registered through a unified command registry
The TUI SHALL register slash commands through a unified dynamic command registry so help output, command dispatch metadata, command validation, and slash suggestions are derived from the same self-describing command definitions.

#### Scenario: /help lists newly added commands
- **WHEN** the user runs `/help`
- **THEN** the help output SHALL include `/diff`, `/cost`, `/config`, `/resume`, `/context`, `/export`, `/copy`, and `/theme` with brief descriptions

#### Scenario: Unknown slash command is rejected consistently
- **WHEN** the user enters an unrecognized slash command
- **THEN** the command dispatcher SHALL return a consistent unknown-command error message

#### Scenario: Slash suggestions reuse registered command definitions
- **WHEN** the input buffer begins with `/`
- **THEN** the suggestion overlay SHALL source command candidates from the same registry used by `/help` and command dispatch

#### Scenario: Command registry supports dynamic registration
- **WHEN** a slash command definition is registered at runtime or initialization time
- **THEN** command validation, help output, and slash suggestions SHALL reflect that command without requiring a separate static command list update
