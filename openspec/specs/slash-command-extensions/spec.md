## ADDED Requirements

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
The TUI SHALL register slash commands through a unified command registry so help output and command dispatch are derived from the same command definitions.

#### Scenario: /help lists newly added commands
- **WHEN** the user runs `/help`
- **THEN** the help output SHALL include `/diff`, `/cost`, and `/config` with brief descriptions

#### Scenario: Unknown slash command is rejected consistently
- **WHEN** the user enters an unrecognized slash command
- **THEN** the command dispatcher SHALL return a consistent unknown-command error message

### Requirement: Built-in slash commands include memory inspection
The built-in slash command set SHALL include `/memory` as a first-party command.

#### Scenario: Help output lists memory command
- **WHEN** the user requests slash command help or the command list is rendered
- **THEN** `/memory` appears in the built-in slash command inventory with a description of its purpose

#### Scenario: Command validation recognizes memory command
- **WHEN** slash command parsing validates a built-in command token
- **THEN** `/memory` is accepted as a supported built-in command
