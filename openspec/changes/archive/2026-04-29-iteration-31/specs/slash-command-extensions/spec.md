## ADDED Requirements

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
