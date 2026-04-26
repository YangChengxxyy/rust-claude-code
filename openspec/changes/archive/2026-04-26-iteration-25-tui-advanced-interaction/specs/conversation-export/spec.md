## ADDED Requirements

### Requirement: Current conversation can be exported to Markdown
The TUI SHALL provide a `/export` command that writes the current conversation transcript to a Markdown file.

#### Scenario: Export with explicit path
- **WHEN** the user runs `/export <path>`
- **THEN** the system SHALL write the current conversation transcript to the requested Markdown file path
- **AND** the TUI SHALL display the final exported path on success

#### Scenario: Export without explicit path
- **WHEN** the user runs `/export` without a path
- **THEN** the system SHALL write the transcript to a default Markdown export path derived from the current session id or timestamp
- **AND** the TUI SHALL display the final exported path on success

#### Scenario: Export fails
- **WHEN** the export path cannot be created or written
- **THEN** the TUI SHALL display a clear error and SHALL NOT report success

### Requirement: Markdown export preserves conversation structure
The exported Markdown SHALL preserve the readable structure of the current conversation.

#### Scenario: Export includes metadata
- **WHEN** a transcript is exported
- **THEN** the Markdown file SHALL include session metadata such as model, working directory, created or exported timestamp, and message count when available

#### Scenario: Export includes user and assistant messages
- **WHEN** the current conversation contains user and assistant messages
- **THEN** the Markdown file SHALL include those messages in chronological order with clear role headings

#### Scenario: Export includes tool interactions
- **WHEN** the current conversation contains tool use or tool result entries
- **THEN** the Markdown file SHALL include those entries using readable headings and fenced blocks where appropriate
