## ADDED Requirements

### Requirement: Micro-compaction of old tool results
The `CompactionService` SHALL provide a `micro_compact()` method that reduces context size by replacing old `ToolResult` content blocks with a placeholder string `[Content cleared to reduce context size]`. This method SHALL NOT require an LLM call.

#### Scenario: Replace old tool results beyond preservation window
- **WHEN** `micro_compact()` is called with a message history of 20 turns and a preservation window of 3 turns
- **THEN** `ToolResult` content blocks in turns 1-17 SHALL be replaced with `[Content cleared to reduce context size]`, and turns 18-20 SHALL retain their original `ToolResult` content

#### Scenario: Preservation window default
- **WHEN** `micro_compact()` is called without a custom preservation window
- **THEN** the default preservation window SHALL be 3 turns (the most recent 3 assistant-user turn pairs)

#### Scenario: Only ToolResult blocks are affected
- **WHEN** `micro_compact()` processes messages containing both `Text` and `ToolResult` content blocks in old turns
- **THEN** only `ToolResult` content blocks SHALL be replaced; `Text`, `Thinking`, and `ToolUse` blocks SHALL remain unchanged

#### Scenario: Tool result types targeted for clearing
- **WHEN** `micro_compact()` encounters `ToolResult` blocks from Bash, FileRead, Grep, Glob, WebSearch, or WebFetch tools in old turns
- **THEN** their content SHALL be replaced with the placeholder string

#### Scenario: Empty or already-cleared tool results
- **WHEN** `micro_compact()` encounters a `ToolResult` that already contains the placeholder text or is empty
- **THEN** it SHALL leave the block unchanged (no double-replacement)

### Requirement: Micro-compaction result reporting
The `micro_compact()` method SHALL return a `MicroCompactionResult` indicating how many tool result blocks were cleared and the estimated token reduction.

#### Scenario: Result after micro-compaction
- **WHEN** `micro_compact()` clears 15 tool result blocks totaling approximately 80,000 estimated tokens of content
- **THEN** the result SHALL report `blocks_cleared: 15` and an estimated token reduction of approximately 80,000

#### Scenario: No blocks to clear
- **WHEN** `micro_compact()` finds no tool result blocks eligible for clearing (all within preservation window or already cleared)
- **THEN** the result SHALL report `blocks_cleared: 0` and token reduction of 0

### Requirement: Micro-compaction operates in-place on AppState
The `micro_compact()` method SHALL modify the messages in `AppState` in-place, consistent with how `compact()` operates. It SHALL acquire the `AppState` mutex lock, modify messages, and release the lock.

#### Scenario: AppState messages are modified
- **WHEN** `micro_compact()` completes successfully
- **THEN** `AppState.messages` SHALL reflect the cleared tool results immediately, without requiring the caller to copy results back

#### Scenario: Concurrent access safety
- **WHEN** `micro_compact()` is called while the AppState mutex is not held by another task
- **THEN** it SHALL acquire the lock, perform modifications, and release the lock within a single critical section
