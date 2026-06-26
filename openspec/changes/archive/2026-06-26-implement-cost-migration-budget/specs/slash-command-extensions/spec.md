## MODIFIED Requirements

### Requirement: /cost command shows cumulative session usage
The TUI SHALL support a `/cost` slash command that displays cumulative session token usage, model-aware cost estimates, cache token cost categories, configured budget status, and per-model cost breakdowns based on recorded usage totals.

#### Scenario: /cost after one completed turn
- **WHEN** the session has recorded usage from at least one completed assistant turn and the user runs `/cost`
- **THEN** the output SHALL show cumulative input tokens, output tokens, cache read tokens, cache creation tokens, total cost, and the model pricing family used for the estimate

#### Scenario: /cost with no usage yet
- **WHEN** the user runs `/cost` before any usage has been recorded
- **THEN** the output SHALL report zero or unavailable usage without failing

#### Scenario: /cost shows per-model breakdown
- **WHEN** the session has recorded usage for more than one model and the user runs `/cost`
- **THEN** the output SHALL show separate token and cost totals for each model plus a combined session total

#### Scenario: /cost shows budget status
- **WHEN** `max_budget_usd` is configured and the user runs `/cost`
- **THEN** the output SHALL show the configured budget, cumulative cost, remaining budget when positive, and whether the status is ok, warning, or exceeded
