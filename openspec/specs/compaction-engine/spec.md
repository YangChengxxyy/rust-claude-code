## ADDED Requirements

### Requirement: Token estimation for message history
The system SHALL provide a function to estimate the token count of the current conversation. The PRIMARY estimation method SHALL use the `input_tokens` field from the most recent API response `Usage` data, combined with a character-based heuristic (characters / 4) for any messages added after the last API call. The FALLBACK method (when no API usage is available) SHALL use the full character-based heuristic for all messages and the system prompt.

#### Scenario: Estimate token count using API usage data
- **WHEN** the last API response reported `input_tokens: 50000` and 2 new messages totaling 4000 characters have been added since
- **THEN** the estimated token count SHALL be approximately 51000 (50000 + 4000/4)

#### Scenario: Estimate token count without API usage data (first turn)
- **WHEN** no API response has been received yet and the message list contains text-only messages totaling 4000 characters
- **THEN** the estimated token count SHALL be approximately 1000 (full character-based fallback)

#### Scenario: Estimate token count including tool use and results
- **WHEN** the system estimates tokens for messages containing ToolUse (with JSON input) and ToolResult blocks and no API usage is available
- **THEN** the estimation SHALL include the serialized JSON content of ToolUse inputs and the text content of ToolResult blocks using the character-based heuristic

#### Scenario: Estimate token count including system prompt
- **WHEN** a system prompt of 2000 characters is provided along with messages and no API usage is available
- **THEN** the total estimate SHALL include approximately 500 tokens for the system prompt plus the message token estimate

### Requirement: Compaction threshold detection
The system SHALL detect when the estimated token count of the current conversation exceeds a configurable threshold. The default threshold SHALL be 80% of the model context window (default context window: 200,000 tokens). Token estimation SHALL use the usage-based method (API `input_tokens` + character heuristic for new messages) when available, falling back to full character heuristic when no API usage data exists.

#### Scenario: Below threshold
- **WHEN** the estimated token count is 100,000 and the threshold is 160,000 (80% of 200,000)
- **THEN** the system SHALL report that compaction is NOT needed

#### Scenario: Above threshold with usage-based counting
- **WHEN** the last API response reported `input_tokens: 155000` and 30000 characters of new messages have been added (estimate: 155000 + 7500 = 162500) and the threshold is 160,000
- **THEN** the system SHALL report that compaction IS needed

### Requirement: Message partitioning for compaction
The system SHALL partition the message history into two segments: messages to be compacted (early history) and messages to be preserved (recent context). The preserved segment SHALL contain the most recent messages whose estimated token count does not exceed 50% of the context window.

#### Scenario: Partition with sufficient recent context
- **WHEN** the message history has 20 turns and the last 8 turns total approximately 80,000 estimated tokens (below 100,000 = 50% of 200,000)
- **THEN** the system SHALL mark the first 12 turns for compaction and preserve the last 8 turns

#### Scenario: Partition with very long recent messages
- **WHEN** the last 3 turns alone exceed 50% of context window
- **THEN** the system SHALL compact all messages except the last 2 turns (minimum preserved count SHALL be 2 messages — one user + one assistant)

#### Scenario: Too few messages to compact
- **WHEN** the total message count is 4 or fewer
- **THEN** the system SHALL skip compaction and return the original messages unchanged

### Requirement: Summary generation via LLM
The system SHALL generate a conversation summary by sending the to-be-compacted messages to the same LLM API (via `ModelClient`) with a compaction-specific prompt. The summary request SHALL use max_tokens of 8192. The compaction prompt SHALL instruct the model to preserve: file paths mentioned, tool calls and their outcomes, key decisions made, and the overall conversation flow.

#### Scenario: Successful summary generation
- **WHEN** the system sends 12 turns of history to the LLM for summarization
- **THEN** the LLM SHALL return a text summary, and the system SHALL construct a new `Message::user` containing `ContentBlock::Text` with the summary prefixed by `[COMPACTED]\n\n`

#### Scenario: Summary generation API failure
- **WHEN** the LLM API call for summarization fails (network error, rate limit, etc.)
- **THEN** the system SHALL return an error and leave the original message history unchanged

### Requirement: Message history replacement after compaction
After successful summary generation, the system SHALL replace the compacted messages in `AppState.messages` with the single summary message, followed by the preserved recent messages. The total message count after compaction SHALL be: 1 (summary) + preserved message count.

#### Scenario: Successful compaction replacement
- **WHEN** 12 messages are compacted and 8 are preserved, and the LLM returns a summary
- **THEN** `AppState.messages` SHALL contain exactly 9 messages: 1 summary message + 8 preserved messages

#### Scenario: Message ordering after compaction
- **WHEN** compaction completes
- **THEN** the summary message SHALL be at index 0, and preserved messages SHALL maintain their original relative order

### Requirement: CompactionConfig type
The system SHALL define a `CompactionConfig` struct in the `core` crate with the following fields: `context_window: u32` (default 200,000), `threshold_ratio: f32` (default 0.8), `preserve_ratio: f32` (default 0.5), `summary_max_tokens: u32` (default 8192).

#### Scenario: Default configuration
- **WHEN** `CompactionConfig::default()` is called
- **THEN** context_window SHALL be 200,000, threshold_ratio SHALL be 0.8, preserve_ratio SHALL be 0.5, summary_max_tokens SHALL be 8192

### Requirement: CompactionResult type
The system SHALL define a `CompactionResult` struct containing: `original_message_count: usize`, `compacted_message_count: usize`, `preserved_message_count: usize`, `estimated_tokens_before: u32`, `estimated_tokens_after: u32`, `summary_length: usize`.

#### Scenario: Result after compaction
- **WHEN** compaction processes 20 messages, compacts 12, preserves 8, and the summary is 3000 characters
- **THEN** the `CompactionResult` SHALL report original_message_count=20, compacted_message_count=12, preserved_message_count=8, and summary_length=3000

### Requirement: Auto-compaction in QueryLoop
The `QueryLoop` SHALL check the compaction threshold before each API call (before `build_request()`). If the threshold is exceeded, it SHALL automatically trigger compaction before proceeding with the request.

#### Scenario: Auto-compaction triggers before API call
- **WHEN** the estimated tokens exceed the threshold at the start of a query loop iteration
- **THEN** the system SHALL perform compaction, then build the request with the compacted history

#### Scenario: Auto-compaction skipped when below threshold
- **WHEN** the estimated tokens are below the threshold
- **THEN** the system SHALL proceed directly to build_request without compaction

### Requirement: Re-inject project guidance after compaction
After successful compaction, the system SHALL preserve project guidance from discovered `CLAUDE.md` content by including a bounded project-guidance section in the compaction context used for continued conversation.

#### Scenario: Project guidance available during compaction
- **WHEN** compaction runs in a project with discovered `CLAUDE.md` content
- **THEN** the compacted conversation context SHALL include a bounded summary or excerpt of that project guidance for subsequent API requests

#### Scenario: Project guidance unavailable during compaction
- **WHEN** no `CLAUDE.md` content is available during compaction
- **THEN** compaction SHALL continue without failing and SHALL omit the project-guidance section

### Requirement: Preserve used MCP tool context after compaction
After successful compaction, the system SHALL preserve a bounded summary of MCP servers and MCP tools that were used during the session when that information is available.

#### Scenario: MCP tools were used before compaction
- **WHEN** compaction runs after one or more MCP tools have been called in the session
- **THEN** the compacted conversation context SHALL include the MCP server names and tool names needed to understand those prior tool interactions

#### Scenario: No MCP tools were used before compaction
- **WHEN** compaction runs without any recorded MCP tool usage
- **THEN** compaction SHALL omit the MCP tool context section

### Requirement: Preserve recent permission decisions after compaction
After successful compaction, the system SHALL preserve a bounded summary of recent permission decisions so the agent can maintain continuity about allowed and denied actions.

#### Scenario: Permission decisions exist before compaction
- **WHEN** recent permission decisions have been recorded before compaction
- **THEN** the compacted conversation context SHALL include a bounded list of recent allow, deny, or ask decisions with associated tool names

#### Scenario: Permission context exceeds bound
- **WHEN** the number of recent permission decisions exceeds the configured bound
- **THEN** compaction SHALL include only the most recent decisions up to the bound

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
