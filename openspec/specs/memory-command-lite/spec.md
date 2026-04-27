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

### Requirement: The `/memory` command output SHALL be tested across store states
The `/memory` command output formatting SHALL be verified for projects with no memory store, an empty store, and a populated store.

#### Scenario: No memory store exists
- **WHEN** `/memory` is run in a directory with no discoverable memory store
- **THEN** the output indicates that no memory store was found

#### Scenario: Empty memory store
- **WHEN** `/memory` is run in a project with a memory directory but no memory entry files
- **THEN** the output shows the store location and reports zero entries

#### Scenario: Populated memory store
- **WHEN** `/memory` is run in a project with one or more memory entry files
- **THEN** the output lists entry names, types, and descriptions

### Requirement: The `/memory remember` command SHALL use dedup-aware writing
When the user invokes `/memory remember`, the system SHALL check for duplicates and update existing entries rather than creating duplicates.

#### Scenario: Remember with dedup hit
- **WHEN** the user runs `/memory remember` and a duplicate is detected
- **THEN** the system updates the existing entry and reports that it was updated

#### Scenario: Remember with no dedup hit
- **WHEN** the user runs `/memory remember` and no duplicate is found
- **THEN** the system creates a new entry as before

### Requirement: Auto-memory writes SHALL share manual memory command persistence behavior
Automatic memory writes SHALL use the same persistence path as `/memory remember`, including typed frontmatter, topic file creation or correction, duplicate detection, and `MEMORY.md` index rebuild.

#### Scenario: Auto-memory write matches existing memory
- **WHEN** an automatic memory write targets a path or topic name that already exists
- **THEN** the system updates the existing memory entry as `/memory remember` would

#### Scenario: Auto-memory write is new
- **WHEN** an automatic memory write does not match an existing memory entry
- **THEN** the system creates a new topic file and updates `MEMORY.md` as `/memory remember` would

### Requirement: Manual memory commands SHALL remain available when auto-memory is disabled
Disabling automatic memory SHALL NOT disable manual `/memory` inspection, `/memory remember`, or `/memory forget` commands.

#### Scenario: Auto-memory disabled and user remembers manually
- **WHEN** `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` is set and the user runs `/memory remember`
- **THEN** the system processes the manual memory write normally

#### Scenario: Auto-memory disabled and user inspects memory
- **WHEN** `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` is set and the user runs `/memory`
- **THEN** the system displays memory store information normally
