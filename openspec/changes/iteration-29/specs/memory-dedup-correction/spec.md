## ADDED Requirements

### Requirement: The system SHALL apply duplicate detection to auto-memory writes
The memory subsystem SHALL run duplicate detection before automatic memory writes and use the same path and topic-name matching rules as manual memory writes.

#### Scenario: Auto-memory path duplicate detected
- **WHEN** an automatic memory write has a `relative_path` matching an existing memory entry
- **THEN** the duplicate detection function returns the matching entry and the system updates it

#### Scenario: Auto-memory topic duplicate detected
- **WHEN** an automatic memory write has a frontmatter name matching an existing entry case-insensitively
- **THEN** the duplicate detection function returns the matching entry and the system updates it

#### Scenario: Auto-memory duplicate not found
- **WHEN** an automatic memory write has no matching path or topic name
- **THEN** the system creates a new memory entry

### Requirement: Duplicate auto-memory updates SHALL preserve index consistency
When automatic memory updates an existing entry, the memory subsystem SHALL rebuild the memory index after the correction completes.

#### Scenario: Auto-memory corrects duplicate entry
- **WHEN** an automatic memory write updates an existing memory file
- **THEN** the system rebuilds `MEMORY.md` so it points to the updated topic file
