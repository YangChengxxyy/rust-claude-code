## ADDED Requirements

### Requirement: Session lifecycle hook execution
The `HookRunner` SHALL execute `SessionStart` and `SessionEnd` hooks using the same command execution, timeout, environment, and JSON stdin mechanisms as other hook events.

#### Scenario: SessionStart hook input format
- **WHEN** a SessionStart hook fires for a session in `/workspace`
- **THEN** stdin SHALL contain a JSON object with `cwd`, `session_id`, and `event: "SessionStart"`

#### Scenario: SessionEnd hook input format
- **WHEN** a SessionEnd hook fires because a session completed normally
- **THEN** stdin SHALL contain a JSON object with `cwd`, `session_id`, `event: "SessionEnd"`, and a session end reason

### Requirement: Once hook execution
When a matching hook has `once: true`, the `HookRunner` SHALL execute that hook at most once per session. Subsequent matching events in the same session SHALL skip that hook.

#### Scenario: Once hook runs once
- **WHEN** a `SessionStart` hook with `once: true` matches twice in the same session
- **THEN** the hook command SHALL execute only for the first matching event

#### Scenario: Non-once hook runs repeatedly
- **WHEN** a `PreToolUse` hook has `once: false` and matches two tool calls
- **THEN** the hook command SHALL execute for both tool calls

### Requirement: PreToolUse updated input application
For `PreToolUse` hooks, if an approving hook response includes a valid `updatedInput` object, the hook runner result SHALL carry the replacement input to the caller. If multiple matching hooks approve with `updatedInput`, later hooks SHALL receive the most recently updated input and may replace it again.

#### Scenario: Single hook updates input
- **WHEN** a PreToolUse hook for Bash returns `{"decision": "approve", "updatedInput": {"command": "pwd"}}`
- **THEN** the hook result SHALL approve execution with replacement input `{"command": "pwd"}`

#### Scenario: Multiple hooks update input sequentially
- **WHEN** two PreToolUse hooks both approve and return `updatedInput`
- **THEN** the second hook SHALL receive the first hook's updated input and the final hook result SHALL contain the second hook's replacement input

#### Scenario: Blocking hook after update
- **WHEN** a PreToolUse hook updates input and a later PreToolUse hook blocks execution
- **THEN** the final hook result SHALL block execution and no updated input SHALL be applied to tool execution
