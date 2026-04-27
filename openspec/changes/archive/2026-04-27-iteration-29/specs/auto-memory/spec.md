## ADDED Requirements

### Requirement: The system SHALL identify auto-memory candidates from conversation context
The system SHALL instruct the agent to identify durable memory candidates when the user corrects prior behavior, states a stable preference, or provides project context that is not derivable from repository state.

#### Scenario: User corrects agent behavior
- **WHEN** the user corrects how the agent should perform future work
- **THEN** the system treats the correction as an eligible `feedback` memory candidate

#### Scenario: User states stable preference
- **WHEN** the user states a durable preference about communication, tooling, or workflow
- **THEN** the system treats the preference as an eligible `user` memory candidate

#### Scenario: User provides derivable repository information
- **WHEN** the user mentions code structure, file paths, or behavior that can be verified from the repository
- **THEN** the system does not save it as auto memory solely because it appeared in conversation

### Requirement: The system SHALL persist accepted auto-memory candidates through the memory store
Auto-memory writes SHALL use the same topic-file-first memory store format, index rebuild behavior, and typed memory metadata as manual memory writes.

#### Scenario: Auto-memory candidate is accepted
- **WHEN** the agent requests that a durable memory candidate be saved
- **THEN** the system writes or updates a memory topic file and rebuilds the memory index

#### Scenario: Memory store is unavailable
- **WHEN** an auto-memory write is requested but no memory store can be resolved
- **THEN** the system skips the write and reports the memory save as unavailable without failing the user request

### Requirement: Auto-memory SHALL be disabled by environment variable
When `CLAUDE_CODE_DISABLE_AUTO_MEMORY` is set to `1` or `true`, the system SHALL disable automatic memory writes while preserving manual memory inspection and manual memory commands.

#### Scenario: Disable flag is set
- **WHEN** `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` is present and the agent produces an auto-memory candidate
- **THEN** the system does not write or update any memory file for that candidate

#### Scenario: Disable flag is not set
- **WHEN** the disable flag is absent and an eligible auto-memory candidate is accepted
- **THEN** the system may save the candidate through the normal memory write path

### Requirement: Auto-memory SHALL avoid sensitive or transient content
The system SHALL prevent auto-memory guidance from saving secrets, credentials, one-off task state, git history, derivable repository facts, or content the user explicitly says not to remember.

#### Scenario: User says not to remember
- **WHEN** the user explicitly says not to remember a detail
- **THEN** the system does not save that detail through auto-memory

#### Scenario: Candidate includes a secret
- **WHEN** a candidate contains an API key, credential, token, or other secret-like value
- **THEN** the system does not persist the candidate as memory

### Requirement: Auto-memory outcomes SHALL be visible enough for debugging
The system SHALL make auto-memory save, update, skip, and disabled outcomes observable in logs, tool results, or TUI system messages without exposing sensitive candidate content.

#### Scenario: Auto-memory updates an existing entry
- **WHEN** an auto-memory write updates a duplicate memory entry
- **THEN** the user-visible or debug output identifies that memory was updated rather than duplicated

#### Scenario: Auto-memory is disabled
- **WHEN** an auto-memory candidate is skipped because the disable flag is set
- **THEN** the output identifies auto-memory as disabled
