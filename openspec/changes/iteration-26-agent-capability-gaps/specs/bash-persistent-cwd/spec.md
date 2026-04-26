## ADDED Requirements

### Requirement: Bash commands use session-persistent current working directory
The system SHALL maintain a session-persistent current working directory for Bash command execution.

#### Scenario: Bash starts from session CWD by default
- **WHEN** the model invokes `Bash` without a `workdir`
- **THEN** the command SHALL execute with the current session CWD as its working directory

#### Scenario: Explicit workdir controls command start directory
- **WHEN** the model invokes `Bash` with a valid `workdir`
- **THEN** the command SHALL start in that directory for the current invocation

### Requirement: Bash updates session CWD after directory changes
The system SHALL update the session CWD after completed Bash commands when the shell's final working directory can be captured and resolved.

#### Scenario: cd affects later commands
- **WHEN** a Bash command runs `cd /tmp && pwd`
- **THEN** the command output SHALL include `/tmp`
- **AND** the next Bash command without `workdir` SHALL start in `/tmp`

#### Scenario: Nonzero exit after cd can still update CWD
- **WHEN** a Bash command changes directory and then exits with a nonzero status
- **AND** the shell's final working directory is captured successfully
- **THEN** the session CWD SHALL update to the captured final directory

#### Scenario: Timeout does not update CWD
- **WHEN** a Bash command times out
- **THEN** the session CWD SHALL remain unchanged

#### Scenario: Unresolved final directory does not update CWD
- **WHEN** the shell's final working directory cannot be captured or resolved to an accessible directory
- **THEN** the session CWD SHALL remain unchanged

### Requirement: Bash CWD follows normal shell semantics
The system SHALL NOT confine Bash session CWD updates to the project root during iteration 26.

#### Scenario: Directory outside project root
- **WHEN** the model invokes Bash with a command that changes to `/tmp`
- **AND** `/tmp` exists and is accessible
- **THEN** the session CWD MAY become `/tmp`

