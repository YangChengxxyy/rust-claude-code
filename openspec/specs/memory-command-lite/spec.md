## ADDED Requirements

### Requirement: The system SHALL provide a lightweight `/memory` command
The slash command surface SHALL include a `/memory` command for lightweight inspection of the current memory store.

#### Scenario: Memory store exists
- **WHEN** the user runs `/memory` in a project with an available memory store
- **THEN** the system shows that the memory store exists and provides enough information to inspect it

#### Scenario: Memory store does not exist
- **WHEN** the user runs `/memory` in a project with no available memory store
- **THEN** the system reports that memory is unavailable or not yet initialized

### Requirement: The lightweight `/memory` command SHALL expose memory entrypoint information
The command SHALL expose the key entrypoint path or paths relevant to the current memory store.

#### Scenario: Show memory entrypoint
- **WHEN** `/memory` runs successfully
- **THEN** it shows the relevant `MEMORY.md` entrypoint or equivalent memory store location

### Requirement: The lightweight `/memory` command SHALL expose visible memory content summaries
The command SHALL provide a lightweight summary of visible memory entries or memory metadata without requiring full workflow parity.

#### Scenario: Show visible memories
- **WHEN** memory entries are available in the corpus
- **THEN** `/memory` shows their names, descriptions, types, or other lightweight identifying metadata

### Requirement: The lightweight `/memory` command SHALL not require full TypeScript workflow parity
This iteration's `/memory` command SHALL remain an inspection surface rather than a full file chooser, editor launcher, or folder management interface.

#### Scenario: User expects full interactive editing workflow
- **WHEN** `/memory` is used in this iteration
- **THEN** the system provides inspection-oriented behavior without claiming to support the full reference workflow
