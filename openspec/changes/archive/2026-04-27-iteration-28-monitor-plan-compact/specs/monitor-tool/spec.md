## ADDED Requirements

### Requirement: Monitor long-running command output
The system SHALL provide a `MonitorTool` that runs a shell command, observes stdout and stderr, and returns output lines matching a caller-provided regular expression pattern.

#### Scenario: Matched output is returned
- **WHEN** the agent invokes `MonitorTool` with a command that prints lines and a pattern matching one of those lines
- **THEN** the tool result SHALL include the matching line, stream name, and command exit status when the process completes

#### Scenario: Non-matching output is omitted
- **WHEN** the monitored command prints lines that do not match the provided pattern
- **THEN** the tool result SHALL omit those lines and include a count of omitted lines

### Requirement: Monitor timeout and cleanup
The `MonitorTool` SHALL enforce a caller-provided timeout and SHALL terminate the child process if the timeout expires before the process exits.

#### Scenario: Timeout terminates process
- **WHEN** the monitored command is still running after the configured timeout
- **THEN** the tool SHALL terminate the child process and return a timeout result that includes any matched output collected before termination

### Requirement: Monitor permission checks
The `MonitorTool` SHALL require permission for the command it executes using the same command safety expectations as shell command execution.

#### Scenario: Dangerous monitor command is denied
- **WHEN** the permission manager denies the command string for execution
- **THEN** the monitor command SHALL NOT start and the tool result SHALL report that execution was denied

### Requirement: Monitor output bounds
The `MonitorTool` SHALL bound captured monitor output to prevent unbounded memory growth.

#### Scenario: Output exceeds capture limit
- **WHEN** a monitored command produces more matching output than the configured capture limit
- **THEN** the tool result SHALL include only the bounded captured output and report how many matching lines were truncated
