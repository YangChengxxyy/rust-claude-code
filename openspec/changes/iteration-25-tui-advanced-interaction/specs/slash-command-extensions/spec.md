## MODIFIED Requirements

### Requirement: Slash commands are registered through a unified command registry
The TUI SHALL register slash commands through a unified command registry so help output and command dispatch are derived from the same command definitions.

#### Scenario: /help lists newly added commands
- **WHEN** the user runs `/help`
- **THEN** the help output SHALL include `/diff`, `/cost`, `/config`, `/resume`, `/context`, `/export`, `/copy`, and `/theme` with brief descriptions

#### Scenario: Unknown slash command is rejected consistently
- **WHEN** the user enters an unrecognized slash command
- **THEN** the command dispatcher SHALL return a consistent unknown-command error message

## ADDED Requirements

### Requirement: Advanced TUI commands are first-party slash commands
The TUI SHALL recognize `/resume`, `/context`, `/export`, `/copy`, and `/theme` as first-party slash commands.

#### Scenario: Command validation recognizes advanced commands
- **WHEN** slash command parsing validates a built-in command token
- **THEN** `/resume`, `/context`, `/export`, `/copy`, and `/theme` SHALL be accepted as supported built-in commands

#### Scenario: Help output includes usage forms
- **WHEN** the user runs `/help`
- **THEN** the help output SHALL include usage forms for `/resume [session-id]`, `/context`, `/export [path]`, `/copy`, and `/theme [dark|light|custom]`
