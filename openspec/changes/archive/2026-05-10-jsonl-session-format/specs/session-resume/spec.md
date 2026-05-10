## MODIFIED Requirements

### Requirement: Users can resume a specific saved session
The CLI SHALL support resuming a specific saved session by id through `--resume` and `-r`, supporting both `.json` and `.jsonl` session formats.

#### Scenario: Resume by explicit session id
- **WHEN** the user invokes the CLI with `--resume <session-id>`
- **THEN** the system loads that saved session, checking for both `{id}.jsonl` and `{id}.json` files (preferring `.jsonl` if both exist)

#### Scenario: Requested session does not exist
- **WHEN** the user invokes the CLI with an unknown session id
- **THEN** the system exits with a clear error explaining that the requested session could not be found

### Requirement: Existing continue behavior remains unchanged
The system SHALL preserve the existing semantics of `--continue` as resuming the latest session, with the addition of preferring interrupted sessions.

#### Scenario: Continue latest session
- **WHEN** the user invokes the CLI with `--continue`
- **THEN** the system resumes the latest available saved session (preferring interrupted `.jsonl` sessions over completed ones when more recent)

### Requirement: Session listing includes both formats
The `list_recent_sessions()` function SHALL scan for both `.json` and `.jsonl` files in the sessions directory.

#### Scenario: Mixed format session listing
- **WHEN** the sessions directory contains both `.json` and `.jsonl` files
- **THEN** `list_recent_sessions()` SHALL return summaries for all sessions, sorted by `updated_at` regardless of format
