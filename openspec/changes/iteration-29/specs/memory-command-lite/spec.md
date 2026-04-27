## ADDED Requirements

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
