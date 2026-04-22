## ADDED Requirements

### Requirement: Users can resume a specific saved session
The CLI SHALL support resuming a specific saved session by id through `--resume` and `-r`.

#### Scenario: Resume by explicit session id
- **WHEN** the user invokes the CLI with `--resume <session-id>`
- **THEN** the system loads that saved session instead of selecting the most recent session automatically

#### Scenario: Requested session does not exist
- **WHEN** the user invokes the CLI with an unknown session id
- **THEN** the system exits with a clear error explaining that the requested session could not be found

### Requirement: Existing continue behavior remains unchanged
The system SHALL preserve the existing semantics of `--continue` as resuming the latest session.

#### Scenario: Continue latest session
- **WHEN** the user invokes the CLI with `--continue`
- **THEN** the system resumes the latest available saved session using the current continue flow
