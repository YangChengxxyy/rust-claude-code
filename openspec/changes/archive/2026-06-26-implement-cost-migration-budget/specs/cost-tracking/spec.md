## ADDED Requirements

### Requirement: Model pricing lookup
The system SHALL provide model-family pricing lookup for cost calculation. Pricing SHALL distinguish input, output, cache read, and cache creation token rates per million tokens. Model names containing `opus`, `sonnet`, or `haiku` SHALL use their matching family rates, and unknown models SHALL use sonnet rates as the default.

#### Scenario: Opus model pricing is selected
- **WHEN** cost pricing is requested for model `claude-opus-4-0`
- **THEN** the system SHALL return opus pricing for input, output, cache read, and cache creation tokens

#### Scenario: Unknown model uses sonnet pricing
- **WHEN** cost pricing is requested for model `custom-model`
- **THEN** the system SHALL return sonnet pricing as the default pricing family

### Requirement: API usage cost calculation
The system SHALL calculate API call cost from usage tokens and model pricing. The calculation SHALL include input tokens, output tokens, cache read input tokens, and cache creation input tokens when those fields are present in usage data.

#### Scenario: Cost includes all usage token categories
- **WHEN** usage contains input tokens, output tokens, cache read tokens, and cache creation tokens
- **THEN** the calculated cost SHALL include each category multiplied by its model-specific rate

#### Scenario: Missing cache token fields are treated as zero
- **WHEN** usage contains input and output tokens but no cache token fields
- **THEN** the calculated cost SHALL treat cache read and cache creation tokens as zero

### Requirement: Session cost tracking
The system SHALL maintain a session-level cost tracker that records per-turn cost entries, cumulative token totals, cumulative cost, and per-model breakdowns.

#### Scenario: Usage updates cumulative cost
- **WHEN** an assistant turn completes with usage for model `claude-sonnet-4-0`
- **THEN** the cost tracker SHALL append a cost record and update cumulative totals for that model

#### Scenario: Multiple models are tracked separately
- **WHEN** one session records usage for `claude-sonnet-4-0` and later records usage for `claude-haiku-4-0`
- **THEN** the cost tracker SHALL expose separate model breakdowns and a combined session total

### Requirement: Budget status checks
The system SHALL support an optional maximum session budget in USD. When a budget is configured, the cost tracker SHALL report `Ok` below 80% of the budget, `Warning` at or above 80% and below 100%, and `Exceeded` at or above 100%.

#### Scenario: Budget warning threshold is reached
- **WHEN** `max_budget_usd` is `1.00` and cumulative cost becomes `0.80`
- **THEN** the budget status SHALL be `Warning`

#### Scenario: Budget is exceeded
- **WHEN** `max_budget_usd` is `1.00` and cumulative cost becomes `1.00`
- **THEN** the budget status SHALL be `Exceeded`

### Requirement: Budget warnings are user-visible
The system SHALL notify the user when session cost first enters `Warning` or `Exceeded` budget status. The system MUST avoid repeating the same budget status warning for every subsequent usage update.

#### Scenario: Warning is emitted once per status transition
- **WHEN** cumulative cost crosses from `Ok` to `Warning`
- **THEN** the system SHALL emit a user-visible budget warning exactly once for that transition

#### Scenario: Exceeded warning follows warning
- **WHEN** cumulative cost later crosses from `Warning` to `Exceeded`
- **THEN** the system SHALL emit a user-visible budget exceeded warning
