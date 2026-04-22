## ADDED Requirements

### Requirement: HookRunner creation and configuration
The system SHALL provide a `HookRunner` struct that is constructed from merged hook configuration. `HookRunner` SHALL be `Send + Sync` and usable behind `Arc`.

#### Scenario: Create HookRunner with hooks
- **WHEN** merged settings contain hook configurations
- **THEN** `HookRunner::new(hooks_config)` SHALL create a runner with the provided hooks

#### Scenario: Create HookRunner with no hooks
- **WHEN** merged settings contain no hook configurations
- **THEN** `HookRunner::new(empty_config)` SHALL create a runner that returns `Continue` for all events

### Requirement: Hook matcher filtering
The `HookRunner` SHALL filter hooks based on event type and matcher pattern before execution. For `PreToolUse` and `PostToolUse` events, the matcher SHALL be compared against the tool name. A hook group with an empty matcher or no matcher SHALL match all tools. For non-tool events (`UserPromptSubmit`, `Stop`, `Notification`), the matcher SHALL be ignored and all hook groups for that event SHALL execute.

#### Scenario: Matcher matches tool name
- **WHEN** a `PreToolUse` event fires for tool `"Bash"` and a hook group has `matcher: "Bash"`
- **THEN** the hooks in that group SHALL execute

#### Scenario: Matcher does not match tool name
- **WHEN** a `PreToolUse` event fires for tool `"FileRead"` and a hook group has `matcher: "Bash"`
- **THEN** the hooks in that group SHALL NOT execute

#### Scenario: Empty matcher matches all tools
- **WHEN** a `PreToolUse` event fires for any tool and a hook group has `matcher: ""`
- **THEN** the hooks in that group SHALL execute

#### Scenario: Matcher ignored for non-tool events
- **WHEN** a `UserPromptSubmit` event fires and a hook group has `matcher: "Bash"`
- **THEN** the hooks in that group SHALL still execute (matcher ignored)

### Requirement: Shell command execution
The `HookRunner` SHALL execute hook commands via `tokio::process::Command` using the system shell (`$SHELL` on Unix, `sh` as fallback). The command string SHALL be passed as a single argument to the shell's `-c` flag.

#### Scenario: Execute simple command
- **WHEN** a hook with command `"echo ok"` is triggered
- **THEN** the system SHALL execute `$SHELL -c "echo ok"` and capture stdout/stderr

#### Scenario: Command receives JSON input via stdin
- **WHEN** a hook command is executed
- **THEN** the system SHALL write a JSON object to the command's stdin containing the hook input context

#### Scenario: Environment variables are set
- **WHEN** a hook command is executed
- **THEN** the process environment SHALL include `CLAUDE_PROJECT_DIR` (set to the current working directory) and `HOOK_EVENT` (set to the event name string)

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

#### Scenario: PreToolUse input format
- **WHEN** a PreToolUse hook fires for tool "Bash" with input `{"command": "ls"}`
- **THEN** stdin SHALL contain a JSON object with `tool_name: "Bash"`, `tool_input: {"command": "ls"}`, `cwd`, and `session_id`

#### Scenario: PostToolUse input format
- **WHEN** a PostToolUse hook fires for tool "Bash" with output "file1.txt\nfile2.txt"
- **THEN** stdin SHALL contain a JSON object with `tool_name`, `tool_input`, `tool_output`, `tool_is_error`, `cwd`, and `session_id`

### Requirement: Hook timeout enforcement
The system SHALL enforce a timeout on hook command execution. The default timeout SHALL be 10 seconds. If a hook config specifies a `timeout` field, that value (in seconds) SHALL be used instead. When a hook exceeds its timeout, the process SHALL be killed and the hook SHALL be treated as a non-blocking failure.

#### Scenario: Hook completes within timeout
- **WHEN** a hook command completes in 2 seconds with a 10-second timeout
- **THEN** the system SHALL process the hook's stdout normally

#### Scenario: Hook exceeds timeout
- **WHEN** a hook command does not complete within its timeout period
- **THEN** the system SHALL kill the process, log a warning, and treat the result as approve/continue (non-blocking)

#### Scenario: Custom timeout from config
- **WHEN** a hook config has `timeout: 30`
- **THEN** the system SHALL use 30 seconds as the timeout instead of the default 10

### Requirement: Hook result parsing for PreToolUse
For `PreToolUse` hooks, the system SHALL parse the command's stdout as JSON. The expected format is:
- `decision` (optional string): `"approve"` or `"block"`
- `reason` (optional string): explanation for the decision

If `decision` is `"block"`, the tool execution SHALL be prevented and the `reason` SHALL be reported to the model as a tool error.

#### Scenario: Hook approves tool execution
- **WHEN** a PreToolUse hook outputs `{"decision": "approve"}`
- **THEN** the tool SHALL proceed with execution

#### Scenario: Hook blocks tool execution
- **WHEN** a PreToolUse hook outputs `{"decision": "block", "reason": "unsafe command"}`
- **THEN** the tool SHALL NOT execute and the model SHALL receive an error: "Hook blocked: unsafe command"

#### Scenario: Hook returns empty stdout
- **WHEN** a PreToolUse hook exits with code 0 but produces no stdout
- **THEN** the system SHALL treat it as approve (default)

#### Scenario: Hook returns invalid JSON
- **WHEN** a PreToolUse hook outputs non-JSON text to stdout
- **THEN** the system SHALL log a warning and treat it as approve (default)

#### Scenario: Hook exits with non-zero code
- **WHEN** a PreToolUse hook exits with code 2
- **THEN** the system SHALL treat it as a blocking error and prevent tool execution

#### Scenario: Hook exits with code 1
- **WHEN** a PreToolUse hook exits with code 1
- **THEN** the system SHALL log a warning but treat it as approve (non-blocking)

### Requirement: Multiple hooks execution order
When multiple hooks match the same event, the system SHALL execute them sequentially in configuration order (user hooks before project hooks, within each layer in array order). If any PreToolUse hook returns `block`, subsequent hooks for that event SHALL be skipped and the tool SHALL be blocked.

#### Scenario: All hooks approve
- **WHEN** two PreToolUse hooks both return `{"decision": "approve"}`
- **THEN** the tool SHALL proceed with execution

#### Scenario: First hook blocks
- **WHEN** the first PreToolUse hook returns `{"decision": "block"}` and a second hook exists
- **THEN** the second hook SHALL NOT execute and the tool SHALL be blocked

#### Scenario: Sequential execution order
- **WHEN** user settings has hook A and project settings has hook B for the same event
- **THEN** hook A SHALL execute before hook B
