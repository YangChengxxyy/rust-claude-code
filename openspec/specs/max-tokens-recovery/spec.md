## ADDED Requirements

### Requirement: Detect max tokens truncation
The `QueryLoop` SHALL detect when an API response has `stop_reason == MaxTokens`, indicating the output was truncated due to the token limit.

#### Scenario: Normal end_turn response
- **WHEN** the API response has `stop_reason: "end_turn"`
- **THEN** the QueryLoop SHALL NOT trigger recovery and SHALL return the response normally

#### Scenario: Tool use stop reason
- **WHEN** the API response has `stop_reason: "tool_use"`
- **THEN** the QueryLoop SHALL proceed to tool execution without triggering recovery

#### Scenario: Max tokens stop reason detected
- **WHEN** the API response has `stop_reason: "max_tokens"`
- **THEN** the QueryLoop SHALL enter the recovery flow

### Requirement: Inject continuation message for recovery
When max tokens truncation is detected, the system SHALL:
1. Add the truncated assistant message to the conversation history
2. Inject a user message instructing the model to continue from where it left off
3. Send a new API request with the updated conversation

The continuation message SHALL instruct the model to resume directly without repeating prior content.

#### Scenario: Continuation message content
- **WHEN** the system injects a recovery message
- **THEN** the message SHALL be a user message containing text that instructs continuation without repetition (e.g., "Continue from where you left off. Do not repeat what you already said. Pick up mid-thought if needed.")

#### Scenario: Conversation state after recovery injection
- **WHEN** the recovery message is injected after a truncated assistant response
- **THEN** `AppState.messages` SHALL contain: [...prior messages, truncated assistant message, recovery user message]

### Requirement: Recovery attempt limit
The system SHALL limit max tokens recovery attempts to a maximum of 3 retries per query loop invocation. After 3 recovery attempts, the system SHALL return the last truncated response without further recovery.

#### Scenario: Recovery succeeds on first retry
- **WHEN** the first recovery attempt returns `stop_reason: "end_turn"`
- **THEN** the system SHALL return the completed response and reset the recovery counter

#### Scenario: Recovery exhausts all retries
- **WHEN** all 3 recovery attempts still return `stop_reason: "max_tokens"`
- **THEN** the system SHALL return the last truncated response and log a warning

#### Scenario: Recovery counter resets between user messages
- **WHEN** a new user message is submitted after a previous recovery cycle
- **THEN** the recovery counter SHALL reset to 0, allowing 3 fresh recovery attempts

### Requirement: Recovery with tool use in truncated response
When a truncated response contains both text and tool_use blocks, the system SHALL only trigger recovery if the last content block is a text block that appears truncated. If the response contains completed tool_use blocks, the system SHALL execute those tools first, as they may have been fully formed before truncation.

#### Scenario: Truncated text-only response
- **WHEN** a truncated response contains only text blocks
- **THEN** the system SHALL trigger recovery

#### Scenario: Truncated response with completed tool_use
- **WHEN** a truncated response contains a completed tool_use block followed by truncated text
- **THEN** the system SHALL execute the tool_use first, then include the tool result in the continuation

### Requirement: TUI notification of recovery
When max tokens recovery is triggered, the TUI SHALL display a brief status notification informing the user that output was truncated and the model is continuing.

#### Scenario: Recovery notification displayed
- **WHEN** the QueryLoop detects `stop_reason: max_tokens` and begins recovery
- **THEN** the TUI SHALL show a status message (e.g., "Output truncated, continuing...")

#### Scenario: Recovery counter visible
- **WHEN** recovery is in progress
- **THEN** the TUI status SHALL indicate the attempt number (e.g., "Continuing... (attempt 2/3)")
