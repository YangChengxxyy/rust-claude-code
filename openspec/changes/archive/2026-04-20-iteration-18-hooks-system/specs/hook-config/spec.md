## ADDED Requirements

### Requirement: Hook event type enumeration
The system SHALL define a `HookEvent` enumeration with the following variants: `PreToolUse`, `PostToolUse`, `UserPromptSubmit`, `Stop`, `Notification`. Each variant SHALL have a string representation matching the TS version's event names (PascalCase).

#### Scenario: Serialize hook event to string
- **WHEN** a `HookEvent::PreToolUse` variant is converted to string
- **THEN** the result SHALL be `"PreToolUse"`

#### Scenario: Deserialize hook event from settings
- **WHEN** settings.json contains a `hooks` field with key `"PostToolUse"`
- **THEN** the system SHALL parse it as `HookEvent::PostToolUse`

#### Scenario: Unknown event name in settings
- **WHEN** settings.json contains a `hooks` field with an unrecognized event key (e.g., `"SubagentStart"`)
- **THEN** the system SHALL log a warning and skip that event's hooks without error

### Requirement: Hook configuration model
The system SHALL define a `HookConfig` structure that captures a single hook definition with the following fields:
- `type_` (string): hook type, MUST be `"command"` for this iteration
- `command` (string): shell command to execute
- `matcher` (optional string): tool name pattern for filtering (used by PreToolUse/PostToolUse)
- `timeout` (optional u64): execution timeout in seconds, defaults to 10

#### Scenario: Parse minimal hook config
- **WHEN** settings.json contains `{"type": "command", "command": "echo ok"}`
- **THEN** the system SHALL parse it as a `HookConfig` with `type_="command"`, `command="echo ok"`, `matcher=None`, `timeout=None`

#### Scenario: Parse full hook config
- **WHEN** settings.json contains `{"type": "command", "command": "/usr/local/bin/check.sh", "matcher": "Bash", "timeout": 30}`
- **THEN** the system SHALL parse all fields correctly

#### Scenario: Unsupported hook type
- **WHEN** settings.json contains a hook with `"type": "prompt"`
- **THEN** the system SHALL log a warning and skip that hook entry without error

### Requirement: Hook event group configuration
The system SHALL define a `HookEventGroup` structure that pairs a `matcher` pattern with a list of `HookConfig` entries. The settings.json `hooks` field SHALL map event names to arrays of `HookEventGroup`.

#### Scenario: Parse event group with matcher
- **WHEN** settings.json contains `"PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "check.sh"}]}]`
- **THEN** the system SHALL parse one `HookEventGroup` with `matcher="Bash"` and one command hook

#### Scenario: Parse event group without matcher
- **WHEN** settings.json contains `"PreToolUse": [{"matcher": "", "hooks": [{"type": "command", "command": "log.sh"}]}]`
- **THEN** the system SHALL treat empty matcher as matching all tools

### Requirement: Settings integration for hooks
The `ClaudeSettings` struct SHALL include a `hooks` field of type `HashMap<String, Vec<HookEventGroup>>`. The field SHALL be optional and default to empty.

#### Scenario: Load settings with hooks
- **WHEN** user's `~/.claude/settings.json` contains a `hooks` field with valid hook config
- **THEN** `ClaudeSettings::load()` SHALL populate the `hooks` field correctly

#### Scenario: Load settings without hooks
- **WHEN** settings.json does not contain a `hooks` field
- **THEN** `ClaudeSettings::load()` SHALL default `hooks` to an empty map

### Requirement: Hook config merging across settings layers
When merging user-level and project-level settings, hooks for the same event SHALL be concatenated (project hooks appended after user hooks). Hooks SHALL NOT overwrite each other.

#### Scenario: Merge user and project hooks for same event
- **WHEN** user settings has `PreToolUse: [group_a]` and project settings has `PreToolUse: [group_b]`
- **THEN** the merged result SHALL contain `PreToolUse: [group_a, group_b]` in order

#### Scenario: Merge hooks for different events
- **WHEN** user settings has `PreToolUse: [group_a]` and project settings has `PostToolUse: [group_b]`
- **THEN** the merged result SHALL contain both events independently

#### Scenario: Merge when one layer has no hooks
- **WHEN** user settings has hooks and project settings has no hooks field
- **THEN** the merged result SHALL contain only the user hooks
