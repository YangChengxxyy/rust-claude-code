## MODIFIED Requirements

### Requirement: Hook input context format
The system SHALL pass a JSON object to hook commands via stdin. The base input SHALL include:
- `session_id` (string): current session identifier (or empty if not available)
- `cwd` (string): current working directory

For `PreToolUse`, the input SHALL additionally include:
- `tool_name` (string): name of the tool being executed
- `tool_input` (object): the tool's input parameters

For `PostToolUse`, the input SHALL additionally include:
- `tool_name` (string): name of the tool that was executed
- `tool_input` (object): the tool's input parameters
- `tool_output` (string): the tool's output text
- `tool_is_error` (bool): whether the tool execution resulted in an error

For `UserPromptSubmit`, the input SHALL additionally include:
- `user_message` (string): the user's prompt text

For `Stop`, the input SHALL additionally include:
- `stop_reason` (string): reason for stopping (e.g., "end_turn", "max_rounds")

For `Notification`, the input SHALL additionally include:
- `message` (string): notification text

For `SessionStart`, the input SHALL additionally include:
- `event` (string): `"SessionStart"`
- `model` (string): active model name
- `permission_mode` (string): active permission mode

For `SessionEnd`, the input SHALL additionally include:
- `event` (string): `"SessionEnd"`
- `reason` (string): session end reason
- `duration_secs` (number): elapsed session duration in seconds
- `total_cost_usd` (number): cumulative tracked session cost in USD
- `messages_count` (number): number of messages in the session transcript

#### Scenario: PreToolUse input format
- **WHEN** a PreToolUse hook fires for tool "Bash" with input `{"command": "ls"}`
- **THEN** stdin SHALL contain a JSON object with `tool_name: "Bash"`, `tool_input: {"command": "ls"}`, `cwd`, and `session_id`

#### Scenario: PostToolUse input format
- **WHEN** a PostToolUse hook fires for tool "Bash" with output "file1.txt\nfile2.txt"
- **THEN** stdin SHALL contain a JSON object with `tool_name`, `tool_input`, `tool_output`, `tool_is_error`, `cwd`, and `session_id`

#### Scenario: SessionStart hook input format
- **WHEN** a SessionStart hook fires for a session in `/workspace`
- **THEN** stdin SHALL contain a JSON object with `cwd`, `session_id`, `event: "SessionStart"`, `model`, and `permission_mode`

#### Scenario: SessionEnd hook input format
- **WHEN** a SessionEnd hook fires because a session completed normally
- **THEN** stdin SHALL contain a JSON object with `cwd`, `session_id`, `event: "SessionEnd"`, `reason`, `duration_secs`, `total_cost_usd`, and `messages_count`

### Requirement: Session lifecycle hook execution
The `HookRunner` SHALL execute `SessionStart` and `SessionEnd` hooks using the same command execution, timeout, environment, JSON stdin, and matcher handling mechanisms as other hook events. The CLI SHALL trigger `SessionStart` once after session configuration is resolved and SHALL trigger `SessionEnd` once on normal session shutdown.

#### Scenario: CLI fires SessionStart
- **WHEN** a CLI session begins and SessionStart hooks are configured
- **THEN** the matching SessionStart hooks SHALL execute before user prompts are processed

#### Scenario: CLI fires SessionEnd
- **WHEN** a CLI session exits normally and SessionEnd hooks are configured
- **THEN** the matching SessionEnd hooks SHALL execute with final session metadata

### Requirement: Once hook execution
When a matching hook has `once: true`, the `HookRunner` SHALL execute that hook at most once per session. Subsequent matching events in the same session SHALL skip that hook. Once execution state SHALL be scoped to the `HookRunner` instance and SHALL not persist across process restarts.

#### Scenario: Once hook runs once
- **WHEN** a `SessionStart` hook with `once: true` matches twice in the same session
- **THEN** the hook command SHALL execute only for the first matching event

#### Scenario: Non-once hook runs repeatedly
- **WHEN** a `PreToolUse` hook has `once: false` and matches two tool calls
- **THEN** the hook command SHALL execute for both tool calls
