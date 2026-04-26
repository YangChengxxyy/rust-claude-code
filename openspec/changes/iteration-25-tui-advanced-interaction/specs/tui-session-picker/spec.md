## ADDED Requirements

### Requirement: Recent sessions are listed with useful metadata
The TUI session picker SHALL display recent saved sessions using metadata sufficient for selection, including session id, updated time, model or model setting, message count, working directory, and first user message summary.

#### Scenario: Display recent sessions
- **WHEN** the user opens the session picker and saved sessions exist
- **THEN** the TUI SHALL show a selectable list of recent sessions sorted from newest to oldest
- **AND** each row SHALL include the session id, updated time, model or model setting, message count, working directory, and first user message summary

#### Scenario: Empty session directory
- **WHEN** the user opens the session picker and no saved sessions exist
- **THEN** the TUI SHALL display a clear empty-state message instead of an empty list

#### Scenario: Corrupt session file during listing
- **WHEN** a saved session file cannot be parsed while building the recent session list
- **THEN** the listing SHALL skip that file and continue showing other valid sessions

### Requirement: Session picker supports keyboard navigation
The TUI session picker SHALL support keyboard navigation without modifying the current input draft.

#### Scenario: Move selection
- **WHEN** the session picker is open and the user presses Up or Down
- **THEN** the selected session row SHALL move within the available list and remain visible

#### Scenario: Page through sessions
- **WHEN** the session picker is open and the user presses PageUp or PageDown
- **THEN** the picker SHALL scroll by a page of visible session rows

#### Scenario: Cancel picker
- **WHEN** the session picker is open and the user presses Escape
- **THEN** the picker SHALL close without resuming a session and without changing the current input draft

### Requirement: Selecting a session resumes it
The TUI session picker SHALL resume the selected saved session when the user confirms selection.

#### Scenario: Confirm selected session
- **WHEN** the session picker is open and the user presses Enter on a session row
- **THEN** the TUI SHALL request that session id be resumed
- **AND** the visible transcript and status bar SHALL update to the restored session after resume succeeds

#### Scenario: Selected session no longer exists
- **WHEN** the user selects a session that has been deleted since the picker list was loaded
- **THEN** the TUI SHALL show a clear error and remain in the current session

#### Scenario: Active stream blocks resume
- **WHEN** an assistant response or tool execution is actively streaming
- **THEN** the TUI SHALL NOT resume another session and SHALL tell the user to cancel or wait for the active stream first
