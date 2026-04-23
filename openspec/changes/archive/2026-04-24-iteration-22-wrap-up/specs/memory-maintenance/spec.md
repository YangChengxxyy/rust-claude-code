## MODIFIED Requirements

### Requirement: The system SHALL avoid duplicate memories
The maintenance workflow SHALL prefer updating existing relevant memory files over creating duplicate memories.

#### Scenario: Existing memory already covers the topic
- **WHEN** the system attempts to save memory for a topic already represented in the corpus
- **THEN** it updates the existing memory instead of creating a duplicate when appropriate

#### Scenario: Duplicate detected by path match
- **WHEN** a memory write targets a `relative_path` that already exists in the store
- **THEN** the system uses the correction path to update the existing file instead of creating a second copy

#### Scenario: Duplicate detected by name match
- **WHEN** a memory write targets a topic name that matches an existing entry (case-insensitive)
- **THEN** the system uses the correction path to update the existing file instead of creating a new file with a different path

### Requirement: The system SHALL support memory correction and forgetting
The maintenance workflow SHALL support removing or updating memory that is stale, incorrect, or explicitly revoked.

#### Scenario: User asks to forget something
- **WHEN** the user explicitly asks for a memory to be forgotten
- **THEN** the system removes or updates the relevant stored memory rather than keeping it intact

#### Scenario: User corrects an existing memory
- **WHEN** the user provides updated content for an existing memory topic
- **THEN** the system overwrites the existing memory file and rebuilds the index
