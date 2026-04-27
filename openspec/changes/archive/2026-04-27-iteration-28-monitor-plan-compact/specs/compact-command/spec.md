## ADDED Requirements

### Requirement: /compact retention strategy argument
The TUI SHALL support optional retention strategy arguments for `/compact` while preserving the existing no-argument behavior.

#### Scenario: Compact with default strategy
- **WHEN** the user types `/compact` with no additional arguments
- **THEN** the system SHALL use the existing default compaction strategy

#### Scenario: Compact with named strategy
- **WHEN** the user types `/compact aggressive` or `/compact preserve-recent`
- **THEN** the system SHALL trigger compaction using the selected named retention strategy

#### Scenario: Compact with unknown strategy
- **WHEN** the user types `/compact` with an unrecognized strategy name
- **THEN** the system SHALL display a user-facing error and SHALL NOT start compaction

### Requirement: /compact help describes strategies
The `/help` output SHALL describe supported `/compact` retention strategy arguments.

#### Scenario: Help includes compact strategies
- **WHEN** the user types `/help`
- **THEN** the help output SHALL include `/compact [default|aggressive|preserve-recent]` or equivalent strategy documentation
