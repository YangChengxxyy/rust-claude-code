## ADDED Requirements

### Requirement: The system SHALL discover a project-scoped memory store
The system SHALL discover a project-scoped memory store using stable project identity rules rather than tying memory to a single transient working directory session.

#### Scenario: Resolve memory store for the current project
- **WHEN** a session starts inside a project with a stable project identity
- **THEN** the system resolves the corresponding memory store path for that project

#### Scenario: No memory store exists yet
- **WHEN** a project has no existing memory store
- **THEN** the system reports that no memory store is available without failing normal operation

### Requirement: The system SHALL treat per-memory Markdown files as the primary memory corpus
The system SHALL use individual Markdown memory files as the primary storage units for durable memory content.

#### Scenario: Scan memory corpus
- **WHEN** the system scans the memory store
- **THEN** it discovers per-memory Markdown files and uses them as the durable memory corpus

### Requirement: The system SHALL parse frontmatter-backed memory metadata
The system SHALL parse frontmatter metadata from memory files and expose at least name, description, and type information when present.

#### Scenario: Parse frontmatter metadata
- **WHEN** a memory file contains frontmatter with `name`, `description`, and `type`
- **THEN** the system exposes that metadata to downstream memory features

### Requirement: The system SHALL treat `MEMORY.md` as a compact memory entrypoint
The system SHALL treat `MEMORY.md` as a compact prompt-facing entrypoint and index, not as the primary location for full memory payloads.

#### Scenario: Load memory entrypoint
- **WHEN** the system loads the project memory entrypoint
- **THEN** it treats `MEMORY.md` as concise index content rather than as the authoritative body of all memory content

#### Scenario: Keep entrypoint compact
- **WHEN** `MEMORY.md` exceeds configured prompt-facing limits
- **THEN** the system truncates the entrypoint content rather than loading it as an unbounded document
