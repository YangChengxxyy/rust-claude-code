## ADDED Requirements

### Requirement: Built-in slash commands include memory inspection
The built-in slash command set SHALL include `/memory` as a first-party command.

#### Scenario: Help output lists memory command
- **WHEN** the user requests slash command help or the command list is rendered
- **THEN** `/memory` appears in the built-in slash command inventory with a description of its purpose

#### Scenario: Command validation recognizes memory command
- **WHEN** slash command parsing validates a built-in command token
- **THEN** `/memory` is accepted as a supported built-in command
