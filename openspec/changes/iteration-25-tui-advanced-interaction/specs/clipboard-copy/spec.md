## ADDED Requirements

### Requirement: Latest assistant response can be copied
The TUI SHALL provide a `/copy` command that copies the latest completed assistant response to the system clipboard.

#### Scenario: Copy latest assistant response
- **WHEN** the user runs `/copy` after at least one completed assistant response exists
- **THEN** the system SHALL place the latest assistant response text on the system clipboard
- **AND** the TUI SHALL display a success message

#### Scenario: No assistant response exists
- **WHEN** the user runs `/copy` before any assistant response exists
- **THEN** the TUI SHALL display a clear message that there is no assistant response to copy

#### Scenario: Clipboard provider unavailable
- **WHEN** the user runs `/copy` and the system clipboard cannot be accessed
- **THEN** the TUI SHALL display a clear error and SHALL NOT claim that copy succeeded

### Requirement: Copy ignores transient streaming text
The `/copy` command SHALL copy only completed assistant responses unless the active stream has finished.

#### Scenario: Copy while assistant is streaming
- **WHEN** an assistant response is still streaming and the user runs `/copy`
- **THEN** the system SHALL copy the previous completed assistant response if one exists
- **AND** the TUI SHALL indicate that the active streaming response is not yet copyable
