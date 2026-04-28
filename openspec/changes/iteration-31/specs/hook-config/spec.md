## ADDED Requirements

### Requirement: Session lifecycle hook events
The system SHALL support `SessionStart` and `SessionEnd` as hook event types. Each variant SHALL have a string representation matching its event name in PascalCase.

#### Scenario: Deserialize SessionStart hook event
- **WHEN** settings.json contains a `hooks` field with key `"SessionStart"`
- **THEN** the system SHALL parse it as `HookEvent::SessionStart`

#### Scenario: Deserialize SessionEnd hook event
- **WHEN** settings.json contains a `hooks` field with key `"SessionEnd"`
- **THEN** the system SHALL parse it as `HookEvent::SessionEnd`

### Requirement: Hook once configuration
The system SHALL allow command hook configuration to include an optional boolean `once` field. When omitted, `once` SHALL default to `false`.

#### Scenario: Parse once hook config
- **WHEN** settings.json contains `{"type": "command", "command": "init.sh", "once": true}`
- **THEN** the system SHALL parse the hook config with `once` set to true

#### Scenario: Omitted once defaults false
- **WHEN** settings.json contains `{"type": "command", "command": "log.sh"}`
- **THEN** the system SHALL parse the hook config with `once` set to false

### Requirement: PreToolUse updated input response shape
The system SHALL recognize an optional `updatedInput` object in a `PreToolUse` hook JSON response. `updatedInput` SHALL represent the full replacement input object for the pending tool call.

#### Scenario: Parse hook response with updatedInput
- **WHEN** a PreToolUse hook outputs `{"decision": "approve", "updatedInput": {"command": "ls -la"}}`
- **THEN** the system SHALL parse the hook response as approved with replacement tool input `{"command": "ls -la"}`

#### Scenario: Ignore non-object updatedInput
- **WHEN** a PreToolUse hook outputs `{"decision": "approve", "updatedInput": "invalid"}`
- **THEN** the system SHALL ignore `updatedInput` and treat the hook response as approved without input mutation
