## ADDED Requirements

### Requirement: Model-aware pricing lookup
The system SHALL provide model-aware pricing for Opus, Sonnet, and Haiku model families with separate per-million-token rates for input, output, cache-read, and cache-creation tokens. Unknown model names SHALL use Sonnet pricing.

#### Scenario: Opus pricing is selected
- **WHEN** cost calculation is requested for a model name containing `opus`
- **THEN** the system SHALL use Opus input, output, cache-read, and cache-creation rates

#### Scenario: Unknown pricing defaults to Sonnet
- **WHEN** cost calculation is requested for an unrecognized model name
- **THEN** the system SHALL use Sonnet rates

### Requirement: Usage cost calculation
The system SHALL calculate API call cost from usage tokens and model pricing, including input tokens, output tokens, cache-read input tokens, and cache-creation input tokens when present.

#### Scenario: Cache read tokens use cache pricing
- **WHEN** a usage record contains cache-read input tokens
- **THEN** those tokens SHALL be charged at the model's cache-read rate rather than the normal input rate

#### Scenario: Missing cache fields are zero
- **WHEN** a usage record does not include cache-read or cache-creation token counts
- **THEN** the system SHALL calculate cost using zero for those cache categories

### Requirement: Session cost tracking
The system SHALL track per-call cost records and cumulative session totals in memory for the active process.

#### Scenario: Record two calls
- **WHEN** two usage records are added to the tracker for the same session
- **THEN** the tracker SHALL report a total cost equal to the sum of both calculated call costs

### Requirement: Budget status reporting
The system SHALL support an optional maximum budget in USD and report whether cumulative cost is within budget, near budget, or exceeded.

#### Scenario: Budget exceeded
- **WHEN** cumulative session cost is greater than the configured maximum budget
- **THEN** the budget status SHALL report exceeded

#### Scenario: No budget configured
- **WHEN** no maximum budget is configured
- **THEN** the budget status SHALL report ok
