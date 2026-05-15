## MODIFIED Requirements

### Requirement: /cost command shows cumulative session usage
The TUI SHALL support a `/cost` slash command that displays cumulative session token usage and a cost estimate based on the active model and recorded usage totals. The estimate SHALL use model-aware pricing and SHALL separately account for input, output, cache-read, and cache-creation token categories when those usage fields are available.

#### Scenario: /cost after one completed turn
- **WHEN** the session has recorded usage from at least one completed assistant turn and the user runs `/cost`
- **THEN** the output SHALL show cumulative input tokens, output tokens, cache tokens when present, active model pricing family, and total estimated cost

#### Scenario: /cost with no usage yet
- **WHEN** the user runs `/cost` before any usage has been recorded
- **THEN** the output SHALL report zero or unavailable usage without failing

#### Scenario: /cost includes cache discount
- **WHEN** the session has recorded cache-read input tokens and the user runs `/cost`
- **THEN** the output SHALL show those tokens separately from normal input tokens and SHALL use cache-read pricing for the estimated cost
