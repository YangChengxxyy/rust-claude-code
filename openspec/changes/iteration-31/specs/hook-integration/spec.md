## ADDED Requirements

### Requirement: SessionStart hook integration
The CLI and TUI session startup paths SHALL invoke `SessionStart` hooks after settings and hook configuration are loaded and before the first user prompt is sent to the model.

#### Scenario: SessionStart fires in print mode
- **WHEN** the CLI runs with a prompt argument in non-interactive print mode
- **THEN** `SessionStart` hooks SHALL be invoked before the query loop sends the prompt to the API

#### Scenario: SessionStart fires in TUI mode
- **WHEN** the CLI starts the TUI session
- **THEN** `SessionStart` hooks SHALL be invoked before the first TUI-submitted prompt is sent to the API

### Requirement: SessionEnd hook integration
The CLI and TUI session shutdown paths SHALL invoke `SessionEnd` hooks when a session completes or exits. SessionEnd hooks SHALL be informational and SHALL NOT alter the final assistant response or process exit status unless hook execution itself panics.

#### Scenario: SessionEnd fires after print mode completion
- **WHEN** a non-interactive query completes normally
- **THEN** `SessionEnd` hooks SHALL be invoked before the process returns control to the caller

#### Scenario: SessionEnd fires after TUI exit
- **WHEN** the user exits the TUI session
- **THEN** `SessionEnd` hooks SHALL be invoked during shutdown

#### Scenario: SessionEnd fires after print mode error
- **WHEN** a non-interactive query exits with an error after session startup
- **THEN** `SessionEnd` hooks SHALL still be invoked before the process returns the error

#### Scenario: SessionEnd fires after TUI error
- **WHEN** the TUI session exits with an error after session startup
- **THEN** `SessionEnd` hooks SHALL still be invoked during shutdown

### Requirement: Hook payloads carry the real session identifier
All hook payloads emitted from a session SHALL include the active session's real identifier rather than an empty placeholder. This applies to lifecycle, prompt, stop, and tool hook payloads in both fresh and resumed sessions.

#### Scenario: User prompt hook receives generated session ID
- **WHEN** a new session starts and a user prompt is submitted
- **THEN** the `UserPromptSubmit` hook payload SHALL contain the generated session identifier for that session

#### Scenario: Hook payloads preserve resumed session ID
- **WHEN** the CLI resumes a saved session and later runs tool or stop hooks
- **THEN** those hook payloads SHALL contain the resumed session's persisted identifier

### Requirement: QueryLoop applies updated hook input
The `QueryLoop` SHALL apply approved `PreToolUse` hook `updatedInput` before executing the tool. The applied input SHALL be the input passed to the tool and to subsequent `PostToolUse` hook context.

#### Scenario: Updated input reaches tool execution
- **WHEN** a PreToolUse hook changes Bash input from `{"command": "ls"}` to `{"command": "pwd"}`
- **THEN** the Bash tool SHALL execute with `{"command": "pwd"}`

#### Scenario: Updated input reaches PostToolUse
- **WHEN** a tool executes with input changed by a PreToolUse hook
- **THEN** matching PostToolUse hooks SHALL receive the updated input in their hook context

#### Scenario: No updated input preserves existing behavior
- **WHEN** all matching PreToolUse hooks approve without `updatedInput`
- **THEN** the QueryLoop SHALL execute the tool with the original model-provided input
