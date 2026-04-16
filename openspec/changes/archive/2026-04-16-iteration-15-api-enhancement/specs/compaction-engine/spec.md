## MODIFIED Requirements

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
