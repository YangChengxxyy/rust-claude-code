## ADDED Requirements

### Requirement: Track last API usage in AppState
The `AppState` SHALL track the `Usage` data from the most recent API response. This provides an accurate server-side token count for the conversation up to that point.

#### Scenario: Usage updated after API response
- **WHEN** an API response is received with `usage: { input_tokens: 5000, output_tokens: 800, cache_read_input_tokens: 4200, cache_creation_input_tokens: 0 }`
- **THEN** `AppState` SHALL store this usage data as the most recent API usage

#### Scenario: Usage available for token estimation
- **WHEN** the system needs to estimate the current conversation token count
- **THEN** it SHALL be able to retrieve the most recent `Usage.input_tokens` from `AppState`

### Requirement: Usage-based token estimation function
The system SHALL provide a `estimate_current_tokens()` function that combines the last known API `input_tokens` with a character-based estimate for any messages added since the last API call. The formula SHALL be: `last_api_input_tokens + estimate_tokens(new_messages_since_last_api)`.

#### Scenario: No new messages since last API call
- **WHEN** the last API response reported `input_tokens: 15000` and no new messages have been added
- **THEN** `estimate_current_tokens()` SHALL return 15000

#### Scenario: New messages added since last API call
- **WHEN** the last API response reported `input_tokens: 15000` and 2 new messages totaling 2000 characters have been added
- **THEN** `estimate_current_tokens()` SHALL return approximately 15000 + 500 (2000/4)

#### Scenario: No API call yet (first turn)
- **WHEN** no API response has been received yet (first message in conversation)
- **THEN** `estimate_current_tokens()` SHALL fall back to the full `chars/4` heuristic for all messages plus system prompt

### Requirement: Message index tracking for usage-based counting
The `AppState` SHALL track the message index at which the last API usage was recorded. This allows the system to determine which messages are "new" (added after the last API call) and need character-based estimation.

#### Scenario: Index updated after API response
- **WHEN** `AppState` has 10 messages and an API response is received
- **THEN** the usage message index SHALL be set to 10

#### Scenario: New messages detected
- **WHEN** the usage message index is 10 and `AppState` now has 13 messages
- **THEN** messages at indices 10, 11, 12 SHALL be identified as "new" and subject to character-based estimation
