## Purpose

Define built-in TUI slash command behavior, command registration, command validation, help output, and slash suggestions.
## Requirements
### Requirement: /config command shows effective configuration and sources
The TUI SHALL support a `/config` slash command that displays effective runtime configuration values together with the source of each displayed value.

#### Scenario: /config shows model and source
- **WHEN** the user runs `/config`
- **THEN** the output SHALL include the effective model value and whether it came from CLI arguments, environment variables, project settings, user settings, or defaults

#### Scenario: /config shows permission configuration source
- **WHEN** the runtime permission rules originate from the project settings file
- **THEN** `/config` output SHALL identify project settings as the source of the effective permission configuration

### Requirement: /cost command shows cumulative session usage
The TUI SHALL support a `/cost` slash command that displays cumulative session token usage and a cost estimate based on the active model and recorded usage totals.

#### Scenario: /cost after one completed turn
- **WHEN** the session has recorded usage from at least one completed assistant turn and the user runs `/cost`
- **THEN** the output SHALL show cumulative input tokens, output tokens, and a cost estimate

#### Scenario: /cost with no usage yet
- **WHEN** the user runs `/cost` before any usage has been recorded
- **THEN** the output SHALL report zero or unavailable usage without failing

### Requirement: /diff command shows current workspace diff
The TUI SHALL support a `/diff` slash command that displays the current Git workspace diff when the current working directory is inside a Git repository.

#### Scenario: /diff inside clean repository
- **WHEN** the user runs `/diff` in a clean Git repository
- **THEN** the output SHALL state that there are no working tree changes to display

#### Scenario: /diff inside dirty repository
- **WHEN** the user runs `/diff` in a Git repository with local changes
- **THEN** the output SHALL show the current workspace diff summary or diff content for those changes

#### Scenario: /diff outside repository
- **WHEN** the user runs `/diff` outside a Git repository
- **THEN** the output SHALL report that no Git repository is available

### Requirement: /clear supports preserving conversation context
The TUI SHALL support an enhanced `/clear` command that can clear the visible chat transcript while preserving conversation context when the user explicitly requests that mode.

#### Scenario: Default /clear clears current transcript
- **WHEN** the user runs `/clear` without arguments
- **THEN** the TUI SHALL clear the current visible conversation transcript according to existing clear behavior

#### Scenario: /clear preserve mode keeps context
- **WHEN** the user runs `/clear` with the explicit preserve-context mode
- **THEN** the TUI SHALL clear the visible chat output while preserving the underlying conversation context needed for subsequent turns

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

### Requirement: Built-in slash commands include memory inspection
The built-in slash command set SHALL include `/memory` as a first-party command.

#### Scenario: Help output lists memory command
- **WHEN** the user requests slash command help or the command list is rendered
- **THEN** `/memory` appears in the built-in slash command inventory with a description of its purpose

#### Scenario: Command validation recognizes memory command
- **WHEN** slash command parsing validates a built-in command token
- **THEN** `/memory` is accepted as a supported built-in command

### Requirement: Built-in slash commands SHALL include environment diagnostics
The built-in slash command set SHALL include `/doctor` as a first-party command for diagnosing API, configuration, MCP, tool, and permission health.

#### Scenario: Help output lists doctor command
- **WHEN** the user requests slash command help or the command list is rendered
- **THEN** `/doctor` appears in the built-in slash command inventory with a description of its purpose

#### Scenario: Command validation recognizes doctor command
- **WHEN** slash command parsing validates a built-in command token
- **THEN** `/doctor` is accepted as a supported built-in command

### Requirement: Built-in slash commands SHALL include code review
The built-in slash command set SHALL include `/review` as a first-party command for reviewing current branch or PR changes.

#### Scenario: Help output lists review command
- **WHEN** the user requests slash command help or the command list is rendered
- **THEN** `/review` appears in the built-in slash command inventory with usage that accepts an optional PR number or URL

#### Scenario: Command validation recognizes review command
- **WHEN** slash command parsing validates a built-in command token
- **THEN** `/review` is accepted as a supported built-in command

### Requirement: Slash suggestions present commands and skills as separate groups
The TUI SHALL present slash-triggered suggestions with separate `Commands` and `Skills` groups so users can distinguish executable slash commands from discoverable skills.

#### Scenario: Grouped suggestion headings are shown
- **WHEN** the slash suggestion overlay is rendered with at least one command and one skill candidate
- **THEN** the overlay SHALL show distinct group headings for `Commands` and `Skills`

#### Scenario: Skills are not treated as built-in slash commands
- **WHEN** a skill appears in the suggestion overlay
- **THEN** the TUI SHALL present it as a suggestion source without implicitly registering it as an executable built-in slash command

### Requirement: Built-in slash commands SHALL include custom agent listing
The built-in slash command set SHALL include `/agents` as a first-party command for listing discovered custom agents.

#### Scenario: Help output lists agents command
- **WHEN** the user requests slash command help or the command list is rendered
- **THEN** `/agents` appears in the built-in slash command inventory with a description of its purpose

#### Scenario: Command validation recognizes agents command
- **WHEN** slash command parsing validates a built-in command token
- **THEN** `/agents` is accepted as a supported built-in command

### Requirement: /agents command displays custom agents
The `/agents` command SHALL display loaded custom agents with their names and descriptions. If no custom agents are loaded, it SHALL display a clear empty-state message.

#### Scenario: Display loaded custom agents
- **WHEN** the user runs `/agents` and custom agents `reviewer` and `tester` are loaded
- **THEN** the output SHALL list `reviewer` and `tester` with their descriptions

#### Scenario: Display no custom agents
- **WHEN** the user runs `/agents` and no custom agents are loaded
- **THEN** the output SHALL display `No custom agents configured`

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

