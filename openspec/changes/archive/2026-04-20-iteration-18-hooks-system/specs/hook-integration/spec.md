## ADDED Requirements

### Requirement: PreToolUse hook integration in QueryLoop
The `QueryLoop` SHALL invoke PreToolUse hooks after the permission check passes but before actual tool execution. If a PreToolUse hook blocks the tool, the QueryLoop SHALL skip tool execution and add a tool result with error content indicating the hook blocked it.

#### Scenario: PreToolUse hook allows tool
- **WHEN** a tool passes permission check and all PreToolUse hooks approve
- **THEN** the tool SHALL execute normally

#### Scenario: PreToolUse hook blocks tool
- **WHEN** a tool passes permission check but a PreToolUse hook blocks with reason "forbidden"
- **THEN** the QueryLoop SHALL NOT execute the tool and SHALL add a tool result message with `is_error: true` and content `"Hook blocked: forbidden"`

#### Scenario: No hooks configured
- **WHEN** no hooks are configured and a tool passes permission check
- **THEN** the tool SHALL execute normally (no change from current behavior)

### Requirement: PostToolUse hook integration in QueryLoop
The `QueryLoop` SHALL invoke PostToolUse hooks after tool execution completes (both success and failure). PostToolUse hooks SHALL be informational only — their results SHALL NOT modify the tool result that was already produced.

#### Scenario: PostToolUse hook fires on success
- **WHEN** a tool executes successfully
- **THEN** the PostToolUse hooks matching the tool name SHALL be invoked with the tool's output

#### Scenario: PostToolUse hook fires on tool error
- **WHEN** a tool execution returns an error result
- **THEN** the PostToolUse hooks SHALL still be invoked with `tool_is_error: true`

#### Scenario: PostToolUse hook failure does not affect result
- **WHEN** a PostToolUse hook fails (timeout, crash, or block response)
- **THEN** the tool result already added to the conversation SHALL NOT be affected

### Requirement: UserPromptSubmit hook integration
The system SHALL invoke `UserPromptSubmit` hooks when the user submits a prompt, before the prompt is sent to the API. This applies to both TUI interactive mode and non-interactive (print) mode.

#### Scenario: UserPromptSubmit hook fires on prompt submission
- **WHEN** a user submits a prompt "write a test"
- **THEN** the UserPromptSubmit hooks SHALL be invoked with `user_message: "write a test"` before the API call

#### Scenario: UserPromptSubmit hook failure does not block prompt
- **WHEN** a UserPromptSubmit hook fails
- **THEN** the prompt SHALL still be sent to the API (hooks are informational for this event)

### Requirement: Stop hook integration
The system SHALL invoke `Stop` hooks when the QueryLoop completes normally (stop_reason is `end_turn` or agent loop finishes). Stop hooks SHALL be informational only.

#### Scenario: Stop hook fires on normal completion
- **WHEN** the QueryLoop finishes with `stop_reason: "end_turn"`
- **THEN** Stop hooks SHALL be invoked with `stop_reason: "end_turn"`

#### Scenario: Stop hook fires on max rounds
- **WHEN** the QueryLoop reaches max rounds limit
- **THEN** Stop hooks SHALL be invoked with `stop_reason: "max_rounds"`

### Requirement: HookRunner injection into QueryLoop
The `QueryLoop` SHALL accept an optional `HookRunner` via a builder method (e.g., `with_hook_runner()`). When `HookRunner` is `None`, all hook invocations SHALL be skipped with no overhead.

#### Scenario: QueryLoop with hooks
- **WHEN** `QueryLoop` is constructed with `with_hook_runner(Some(runner))`
- **THEN** hooks SHALL be invoked at the appropriate points during execution

#### Scenario: QueryLoop without hooks
- **WHEN** `QueryLoop` is constructed without calling `with_hook_runner()`
- **THEN** no hook invocations SHALL occur (backward compatible)

### Requirement: TUI hook event notifications
The `TuiBridge` SHALL support a `send_hook_blocked` method that notifies the TUI when a PreToolUse hook blocks a tool. The TUI SHALL display a system message indicating which tool was blocked and the reason.

#### Scenario: Hook block displayed in TUI
- **WHEN** a PreToolUse hook blocks the "Bash" tool with reason "dangerous command"
- **THEN** the TUI SHALL display a system message: "Hook blocked Bash: dangerous command"

### Requirement: /hooks slash command
The system SHALL provide a `/hooks` slash command that displays currently configured hooks. The output SHALL list each event with its hook groups, matchers, and commands.

#### Scenario: Display configured hooks
- **WHEN** user runs `/hooks` and there are 2 PreToolUse hooks configured
- **THEN** the output SHALL show the event name, matcher patterns, and commands for each hook

#### Scenario: No hooks configured
- **WHEN** user runs `/hooks` and no hooks are configured
- **THEN** the output SHALL display "No hooks configured"

### Requirement: Hook execution during concurrent tool batch
For concurrent-safe tools that execute in parallel, PreToolUse hooks SHALL be evaluated for each tool before the parallel batch starts. If any tool is blocked by a hook, only that tool SHALL be skipped — other tools in the batch SHALL proceed. PostToolUse hooks for the batch SHALL run after all concurrent tools complete.

#### Scenario: One tool blocked in concurrent batch
- **WHEN** three concurrent tools are scheduled and a PreToolUse hook blocks one of them
- **THEN** the blocked tool SHALL return an error result, the other two SHALL execute normally in parallel

#### Scenario: PostToolUse hooks after concurrent batch
- **WHEN** three concurrent tools complete successfully
- **THEN** PostToolUse hooks SHALL fire for each tool that executed
