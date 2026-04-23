## ADDED Requirements

### Requirement: The system SHALL detect duplicate memory entries before writing
The memory subsystem SHALL provide a function to check whether an incoming memory write would duplicate an existing entry, based on path or topic name matching.

#### Scenario: Exact path duplicate detected
- **WHEN** a caller requests a memory write with a `relative_path` that matches an existing entry's `relative_path`
- **THEN** the duplicate detection function returns the matching entry

#### Scenario: Topic name duplicate detected
- **WHEN** a caller requests a memory write with a `frontmatter.name` that matches an existing entry's `frontmatter.name` (case-insensitive)
- **THEN** the duplicate detection function returns the matching entry

#### Scenario: No duplicate found
- **WHEN** a caller requests a memory write with no matching path or name in the existing corpus
- **THEN** the duplicate detection function returns no match

### Requirement: The system SHALL support in-place correction of existing memory entries
The memory subsystem SHALL provide a function to update the content of an existing memory file and rebuild the index afterward.

#### Scenario: Correct an existing memory file
- **WHEN** a caller corrects a memory at a `relative_path` that exists in the store
- **THEN** the system overwrites the file content with the new frontmatter and body and rebuilds the `MEMORY.md` index

#### Scenario: Correction target does not exist
- **WHEN** a caller attempts to correct a memory at a `relative_path` that does not exist
- **THEN** the system returns an error indicating the file was not found

### Requirement: The system SHALL provide dedup-aware memory writing
The CLI memory write path SHALL check for duplicates before deciding whether to create a new entry or update an existing one.

#### Scenario: Remember command with existing duplicate
- **WHEN** the user runs `/memory remember` with a path or name that matches an existing memory
- **THEN** the system updates the existing memory instead of creating a new file

#### Scenario: Remember command with no duplicate
- **WHEN** the user runs `/memory remember` with no matching existing memory
- **THEN** the system creates a new memory file as before
