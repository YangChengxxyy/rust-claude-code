## MODIFIED Requirements

### Requirement: Slash commands are registered through a unified command registry
The TUI SHALL register slash commands through a unified command registry so help output, command dispatch, and slash suggestions are derived from the same command definitions.

#### Scenario: /help lists newly added commands
- **WHEN** the user runs `/help`
- **THEN** the help output SHALL include `/diff`, `/cost`, `/config`, `/resume`, `/context`, `/export`, `/copy`, and `/theme` with brief descriptions

#### Scenario: Unknown slash command is rejected consistently
- **WHEN** the user enters an unrecognized slash command
- **THEN** the command dispatcher SHALL return a consistent unknown-command error message

#### Scenario: Slash suggestions reuse registered command definitions
- **WHEN** the input buffer begins with `/`
- **THEN** the suggestion overlay SHALL source command candidates from the same registry used by `/help` and command dispatch

## ADDED Requirements

### Requirement: Slash suggestions present commands and skills as separate groups
The TUI SHALL present slash-triggered suggestions with separate `Commands` and `Skills` groups so users can distinguish executable slash commands from discoverable skills.

#### Scenario: Grouped suggestion headings are shown
- **WHEN** the slash suggestion overlay is rendered with at least one command and one skill candidate
- **THEN** the overlay SHALL show distinct group headings for `Commands` and `Skills`

#### Scenario: Skills are not treated as built-in slash commands
- **WHEN** a skill appears in the suggestion overlay
- **THEN** the TUI SHALL present it as a suggestion source without implicitly registering it as an executable built-in slash command
