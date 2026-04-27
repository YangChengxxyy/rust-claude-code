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

### Requirement: The system SHALL support future automated extraction flows
The maintenance model SHALL be compatible with automated memory extraction and update flows, even if those flows are not fully implemented in the first pass.

#### Scenario: Memory maintenance is invoked outside direct user editing
- **WHEN** a future automated extraction path writes or updates memory
- **THEN** it follows the same topic-file-first and index-update-afterward model

### Requirement: The memory maintenance workflow SHALL support automated extraction flows
The maintenance workflow SHALL accept memory write requests produced by automatic extraction and process them through the same topic-file-first model used by manual memory writes.

#### Scenario: Automated extraction saves project memory
- **WHEN** automatic extraction produces a durable `project` memory candidate
- **THEN** the system writes the candidate to a topic file and updates `MEMORY.md`

#### Scenario: Automated extraction corrects feedback memory
- **WHEN** automatic extraction produces a `feedback` memory candidate that duplicates existing feedback
- **THEN** the system updates the existing topic file and rebuilds the index

### Requirement: Automated memory maintenance SHALL be best-effort
Automatic memory maintenance failures SHALL NOT fail the user-facing response that triggered the memory candidate.

#### Scenario: Auto-memory write fails
- **WHEN** automatic memory maintenance cannot write a candidate because of an I/O error
- **THEN** the system reports or logs the memory write failure while allowing the assistant response to complete

#### Scenario: Auto-memory index rebuild fails
- **WHEN** a topic file write succeeds but index rebuild fails
- **THEN** the system reports or logs the index rebuild failure and leaves the topic file in place
