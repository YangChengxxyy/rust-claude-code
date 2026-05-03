## ADDED Requirements

### Requirement: Detect prompt-too-long API errors
The API client SHALL distinguish prompt-too-long errors from other API errors. When the API returns HTTP 400 with `error.type == "invalid_request_error"` and the error message indicates the prompt is too long (contains "too long", "too many tokens", or "exceeds the maximum"), the error SHALL be mapped to `ApiError::PromptTooLong` instead of the generic `ApiError::Api` variant.

#### Scenario: HTTP 400 with prompt-too-long message
- **WHEN** the API returns HTTP 400 with body `{"error": {"type": "invalid_request_error", "message": "prompt is too long: 250000 tokens > 200000 maximum"}}`
- **THEN** the error SHALL be `ApiError::PromptTooLong` with the error message preserved

#### Scenario: HTTP 400 with non-prompt error
- **WHEN** the API returns HTTP 400 with body `{"error": {"type": "invalid_request_error", "message": "invalid model specified"}}`
- **THEN** the error SHALL be `ApiError::Api { status: 400, message }` (not PromptTooLong)

#### Scenario: HTTP 400 with too-many-tokens message
- **WHEN** the API returns HTTP 400 with body `{"error": {"type": "invalid_request_error", "message": "Request too large: too many tokens"}}`
- **THEN** the error SHALL be `ApiError::PromptTooLong`

### Requirement: Detect overloaded API errors
The API client SHALL distinguish HTTP 529 (overloaded) errors from other API errors. When the API returns HTTP 529, the error SHALL be mapped to `ApiError::Overloaded` instead of the generic `ApiError::Api` variant.

#### Scenario: HTTP 529 overloaded response
- **WHEN** the API returns HTTP 529 with body `{"error": {"type": "overloaded_error", "message": "Overloaded"}}`
- **THEN** the error SHALL be `ApiError::Overloaded` with the error message preserved

#### Scenario: HTTP 429 rate limit (unchanged)
- **WHEN** the API returns HTTP 429
- **THEN** the error SHALL remain `ApiError::RateLimited` (unchanged behavior)

### Requirement: Three-stage reactive compaction on prompt-too-long
The agent loop SHALL implement a three-stage recovery when the API returns `PromptTooLong`. Each stage is attempted once per turn. The stage counter SHALL reset when a new user message is submitted.

#### Scenario: Stage 1 — full LLM compaction succeeds
- **WHEN** the API returns `PromptTooLong` for the first time in a turn
- **THEN** the agent loop SHALL trigger `CompactionService::force_compact()`, and if successful, retry the current turn with the compacted message history

#### Scenario: Stage 1 — full LLM compaction fails, escalate to stage 2
- **WHEN** the API returns `PromptTooLong` for the first time and `force_compact()` fails
- **THEN** the agent loop SHALL immediately attempt stage 2 (micro-compaction) instead of retrying with failed compaction

#### Scenario: Stage 2 — micro-compaction
- **WHEN** the API returns `PromptTooLong` for the second time in a turn (or stage 1 compaction failed)
- **THEN** the agent loop SHALL trigger `CompactionService::micro_compact()` to strip old tool results, and if successful, retry the current turn

#### Scenario: Stage 3 — report error to user
- **WHEN** the API returns `PromptTooLong` for the third time in a turn (or stage 2 also failed)
- **THEN** the agent loop SHALL report the error to the user via `OutputSink::error()` with a message suggesting manual `/compact`, and SHALL NOT retry further

#### Scenario: Stage counter resets on new user message
- **WHEN** a new user message is submitted via `run()`
- **THEN** the `prompt_too_long_stage` counter SHALL reset to 0

### Requirement: RetryState tracking struct
The agent loop SHALL maintain a `RetryState` struct that tracks retry-related state across loop iterations within a single `run()` invocation. This struct SHALL contain at minimum: `prompt_too_long_stage: u8` and `consecutive_overload_count: u32`.

#### Scenario: RetryState initialization
- **WHEN** `QueryLoop::run()` begins a new invocation
- **THEN** a fresh `RetryState` SHALL be created with `prompt_too_long_stage = 0` and `consecutive_overload_count = 0`

#### Scenario: RetryState persists across loop iterations
- **WHEN** the agent loop retries after a recoverable error within the same `run()` invocation
- **THEN** the `RetryState` SHALL retain its accumulated values (e.g., `consecutive_overload_count` carries forward)

### Requirement: Notify user during reactive compaction
The agent loop SHALL notify the user via `OutputSink` when reactive compaction is triggered, so the user understands why there is a delay.

#### Scenario: Notification on stage 1
- **WHEN** stage 1 reactive compaction is triggered
- **THEN** `OutputSink::compaction_start()` SHALL be called before compaction begins

#### Scenario: Notification on stage 2
- **WHEN** stage 2 micro-compaction is triggered
- **THEN** `OutputSink::error()` SHALL be called with a message indicating micro-compaction is being attempted
