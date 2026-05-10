## ADDED Requirements

### Requirement: System detects interrupted sessions
The system SHALL detect `.jsonl` session files that lack a `SessionEnd` event, marking them as interrupted/crashed.

#### Scenario: Session without SessionEnd marker
- **WHEN** a `.jsonl` session file is scanned and no `SessionEnd` event is found
- **THEN** the system SHALL classify that session as "interrupted"

#### Scenario: Session with SessionEnd marker
- **WHEN** a `.jsonl` session file contains a `SessionEnd` event
- **THEN** the system SHALL classify that session as "completed normally"

#### Scenario: Legacy JSON sessions are not flagged
- **WHEN** a `.json` session file is scanned
- **THEN** the system SHALL NOT classify it as interrupted (legacy format has no end marker concept)

### Requirement: Interrupted sessions are surfaced in session list
The `list_recent_sessions()` function SHALL indicate which sessions are interrupted in the returned `SessionSummary`.

#### Scenario: Session summary includes interrupted flag
- **WHEN** `list_recent_sessions()` returns summaries
- **THEN** each `SessionSummary` for an interrupted `.jsonl` session SHALL have an `interrupted` field set to `true`

### Requirement: --continue prefers interrupted sessions
When `--continue` is used and an interrupted session exists, the system SHALL prefer resuming the most recent interrupted session over the most recent completed session.

#### Scenario: Resume most recent interrupted session
- **WHEN** the user invokes `--continue` and an interrupted `.jsonl` session is more recent than the latest completed session
- **THEN** the system SHALL resume that interrupted session

#### Scenario: No interrupted sessions
- **WHEN** the user invokes `--continue` and no interrupted sessions exist
- **THEN** the system SHALL resume the most recent session (current behavior)

### Requirement: Interrupted session loads up to last valid event
When loading an interrupted session, the system SHALL reconstruct state from all valid events up to the point of interruption.

#### Scenario: Partial session recovery
- **WHEN** an interrupted session is loaded
- **THEN** all messages and state from valid events SHALL be restored
- **AND** the session SHALL be ready for continued use with the `SessionWriter` appending to the existing file
