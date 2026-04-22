## ADDED Requirements

### Requirement: The system SHALL write memory payloads to topic files first
When saving or updating memory, the system SHALL treat the topic memory file as the primary payload location.

#### Scenario: Save a new memory
- **WHEN** the system decides to save a durable memory
- **THEN** it writes the memory content to its own topic file instead of writing the full memory body directly into `MEMORY.md`

### Requirement: The system SHALL update `MEMORY.md` mechanically after topic-file changes
The system SHALL update `MEMORY.md` after memory file creation, update, or removal as bookkeeping over the topic files.

#### Scenario: Add a new memory index entry
- **WHEN** a new memory topic file is created
- **THEN** the system updates `MEMORY.md` to include a concise pointer to that file

#### Scenario: Remove a forgotten memory
- **WHEN** a memory topic file is removed as part of forgetting or cleanup
- **THEN** the system removes or updates the corresponding `MEMORY.md` entry

### Requirement: The system SHALL avoid duplicate memories
The maintenance workflow SHALL prefer updating existing relevant memory files over creating duplicate memories.

#### Scenario: Existing memory already covers the topic
- **WHEN** the system attempts to save memory for a topic already represented in the corpus
- **THEN** it updates the existing memory instead of creating a duplicate when appropriate

### Requirement: The system SHALL support memory correction and forgetting
The maintenance workflow SHALL support removing or updating memory that is stale, incorrect, or explicitly revoked.

#### Scenario: User asks to forget something
- **WHEN** the user explicitly asks for a memory to be forgotten
- **THEN** the system removes or updates the relevant stored memory rather than keeping it intact

### Requirement: The system SHALL support future automated extraction flows
The maintenance model SHALL be compatible with automated memory extraction and update flows, even if those flows are not fully implemented in the first pass.

#### Scenario: Memory maintenance is invoked outside direct user editing
- **WHEN** a future automated extraction path writes or updates memory
- **THEN** it follows the same topic-file-first and index-update-afterward model
