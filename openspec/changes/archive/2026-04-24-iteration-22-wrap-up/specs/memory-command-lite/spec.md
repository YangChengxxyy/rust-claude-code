## ADDED Requirements

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
