## ADDED Requirements

### Requirement: Auto permission mode is selectable
The system SHALL support an Auto permission mode through runtime configuration and the `--mode auto` CLI argument.

#### Scenario: CLI selects Auto mode
- **WHEN** the CLI is invoked with `--mode auto`
- **THEN** the effective permission mode SHALL be Auto

#### Scenario: Auto mode appears in parsed permission modes
- **WHEN** configuration parsing reads a permission mode value of `auto`
- **THEN** the system SHALL parse it as `PermissionMode::Auto`

### Requirement: Auto mode approves low-risk operations
The system SHALL automatically approve tool calls in Auto mode when explicit permission rules and safety checks allow the operation.

#### Scenario: Read-only tool is auto-approved
- **WHEN** Auto mode is active and a read-only tool call passes path safety checks
- **THEN** the permission check SHALL allow the tool call without prompting

#### Scenario: Safe file edit is auto-approved
- **WHEN** Auto mode is active and a file edit targets an allowed project path without matching a deny rule
- **THEN** the permission check SHALL allow the file edit without prompting

#### Scenario: Explicit deny still wins
- **WHEN** Auto mode is active and an explicit deny rule matches a tool call
- **THEN** the permission check SHALL deny the tool call

### Requirement: Auto mode escalates risky operations to confirmation
The system SHALL return confirmation-required behavior in Auto mode when safety checks identify a risky operation that is not explicitly denied.

#### Scenario: Dangerous Bash command requires confirmation
- **WHEN** Auto mode is active and Bash input matches dangerous command patterns such as recursive deletion outside allowed paths
- **THEN** the permission check SHALL require confirmation instead of automatically allowing the command

#### Scenario: Unsafe file path requires confirmation
- **WHEN** Auto mode is active and a file tool targets a path outside the configured safe path scope
- **THEN** the permission check SHALL require confirmation instead of automatically allowing the tool call

#### Scenario: Suspicious output is surfaced
- **WHEN** Auto mode approved a tool call and the tool output indicates an abnormal safety condition
- **THEN** the system SHALL surface the safety finding and require confirmation before continuing dependent risky actions

### Requirement: Auto mode composes with sandbox state
The system SHALL consider active sandbox state when evaluating Auto mode safety checks.

#### Scenario: Sandboxed Bash can pass Auto checks
- **WHEN** Auto mode is active, sandboxing is active, and Bash input does not match dangerous command patterns
- **THEN** the command SHALL be eligible for automatic approval

#### Scenario: Sandbox failure blocks Auto approval
- **WHEN** Auto mode is active and sandbox execution is required but unavailable
- **THEN** the tool call SHALL not be automatically approved as if it were sandboxed
