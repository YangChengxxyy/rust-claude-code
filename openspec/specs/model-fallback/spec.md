## ADDED Requirements

### Requirement: Fallback model configuration
The system SHALL support an optional `fallback_model` field in `Config`. The resolution order SHALL be: `RUST_CLAUDE_FALLBACK_MODEL` environment variable → `settings.json` `fallbackModel` field → config.json `fallback_model` field → `None`.

#### Scenario: Fallback model from environment variable
- **WHEN** `RUST_CLAUDE_FALLBACK_MODEL` is set to `"claude-sonnet-4-20250514"`
- **THEN** `Config.fallback_model` SHALL be `Some("claude-sonnet-4-20250514")`

#### Scenario: Fallback model from settings.json
- **WHEN** `RUST_CLAUDE_FALLBACK_MODEL` is not set and `settings.json` contains `"fallbackModel": "claude-haiku-4-20250514"`
- **THEN** `Config.fallback_model` SHALL be `Some("claude-haiku-4-20250514")`

#### Scenario: No fallback model configured
- **WHEN** no fallback model is configured in any source
- **THEN** `Config.fallback_model` SHALL be `None`

### Requirement: Consecutive overload tracking
The agent loop SHALL track the number of consecutive `Overloaded` (HTTP 529) errors received without an intervening successful API response. The counter SHALL be stored in `RetryState.consecutive_overload_count`.

#### Scenario: Increment on overloaded error
- **WHEN** the API returns `ApiError::Overloaded`
- **THEN** `consecutive_overload_count` SHALL increment by 1

#### Scenario: Reset on successful response
- **WHEN** the API returns a successful response after one or more overloaded errors
- **THEN** `consecutive_overload_count` SHALL reset to 0

#### Scenario: Reset does not occur on other errors
- **WHEN** the API returns a non-overloaded error (e.g., `PromptTooLong`, `RateLimited`)
- **THEN** `consecutive_overload_count` SHALL NOT reset (it only resets on success)

### Requirement: Auto-switch to fallback model on consecutive overload
When `consecutive_overload_count` exceeds `MAX_OVERLOAD_RETRIES` (default 3) and a `fallback_model` is configured, the agent loop SHALL switch the model used for API requests to the fallback model for the remainder of the session.

#### Scenario: Switch to fallback after threshold exceeded
- **WHEN** `consecutive_overload_count` reaches 4 (exceeds threshold of 3) and `fallback_model` is `Some("claude-haiku-4-20250514")`
- **THEN** the agent loop SHALL use `"claude-haiku-4-20250514"` as the model for the retry and subsequent API requests in the session

#### Scenario: No switch without configured fallback
- **WHEN** `consecutive_overload_count` exceeds `MAX_OVERLOAD_RETRIES` and `fallback_model` is `None`
- **THEN** the agent loop SHALL continue retrying with the primary model using the existing backoff strategy

#### Scenario: Retry current turn after switch
- **WHEN** the model is switched to the fallback model
- **THEN** the agent loop SHALL immediately retry the current API request with the fallback model

### Requirement: Notify user on model switch
The agent loop SHALL notify the user via `OutputSink` when switching to a fallback model, including the name of the model being switched to.

#### Scenario: Notification message on switch
- **WHEN** the agent loop switches from the primary model to `"claude-haiku-4-20250514"`
- **THEN** `OutputSink::error()` SHALL be called with a message like "Switched to claude-haiku-4-20250514 due to high demand"

#### Scenario: No notification without switch
- **WHEN** an overloaded error occurs but `consecutive_overload_count` has not exceeded the threshold
- **THEN** no model-switch notification SHALL be emitted (standard retry behavior applies)

### Requirement: Overload retry with backoff before fallback
Before each overloaded retry (and before fallback threshold is reached), the agent loop SHALL wait with a brief backoff delay. The delay SHALL be 1 second multiplied by the current `consecutive_overload_count` (e.g., 1s, 2s, 3s).

#### Scenario: Backoff delay on first overloaded retry
- **WHEN** the first `Overloaded` error is received (`consecutive_overload_count` becomes 1)
- **THEN** the agent loop SHALL wait approximately 1 second before retrying

#### Scenario: Backoff delay on third retry
- **WHEN** the third consecutive `Overloaded` error is received (`consecutive_overload_count` becomes 3)
- **THEN** the agent loop SHALL wait approximately 3 seconds before retrying

#### Scenario: No backoff after model switch
- **WHEN** the model has just been switched to the fallback
- **THEN** the retry SHALL proceed immediately without additional backoff delay
