## ADDED Requirements

### Requirement: TUI can resume through an interactive picker
The TUI SHALL support opening an interactive session picker from `/resume` without requiring the user to know a session id.

#### Scenario: Open picker from slash command
- **WHEN** the user runs `/resume` in the TUI without arguments
- **THEN** the TUI SHALL open the recent-session picker

#### Scenario: Resume explicit id from slash command
- **WHEN** the user runs `/resume <session-id>` in the TUI
- **THEN** the system SHALL attempt to resume the specified saved session directly
- **AND** the TUI SHALL display a clear success or not-found message

#### Scenario: CLI resume flags remain unchanged
- **WHEN** the user invokes the CLI with `--resume <session-id>` or `--continue`
- **THEN** the existing CLI resume behavior SHALL remain unchanged
