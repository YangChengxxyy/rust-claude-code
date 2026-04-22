## ADDED Requirements

### Requirement: The system SHALL build recall candidates from memory metadata
The recall pipeline SHALL scan memory file metadata and build a compact candidate manifest before selecting full memories for a query.

#### Scenario: Build memory manifest
- **WHEN** the system prepares memory recall for a user query
- **THEN** it scans memory metadata and builds a manifest of candidate memory files

### Requirement: The system SHALL select relevant memories for a query
The system SHALL select a bounded set of relevant memory files for a query instead of always surfacing the full memory corpus.

#### Scenario: Select relevant memory files
- **WHEN** a query is processed and the memory corpus has available candidates
- **THEN** the system selects only the memory files that are clearly useful for that query

#### Scenario: No memory is clearly relevant
- **WHEN** the candidate memory files do not clearly help with the current query
- **THEN** the system may return no dynamically surfaced memory files

### Requirement: The system SHALL preserve freshness metadata for recalled memory
The recall pipeline SHALL preserve enough freshness metadata to help downstream consumers distinguish recent memory from stale memory.

#### Scenario: Surface freshness metadata
- **WHEN** a memory file is selected for recall
- **THEN** the system carries freshness-related metadata alongside the selected memory

### Requirement: The system SHALL treat recalled memory as historical context
Dynamically recalled memory SHALL be presented as historical context rather than live repository truth.

#### Scenario: Recalled memory is old
- **WHEN** a recalled memory is stale enough to create ambiguity about current state
- **THEN** the system surfaces that memory with cautionary framing instead of presenting it as current fact
