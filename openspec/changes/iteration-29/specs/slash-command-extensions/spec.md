## ADDED Requirements

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
