## ADDED Requirements

### Requirement: TUI shows context usage breakdown
The TUI SHALL provide a `/context` command that displays current context-window usage split into system prompt, conversation messages, tool results, and remaining capacity when the model context capacity is known.

#### Scenario: Show context usage with known capacity
- **WHEN** the user runs `/context` and the active model has a known context capacity
- **THEN** the TUI SHALL display total used tokens, total capacity, remaining tokens, and percentage used
- **AND** the TUI SHALL display a visual bar segmented by system prompt, messages, tool results, and remaining capacity

#### Scenario: Show context usage with unknown capacity
- **WHEN** the user runs `/context` and the active model context capacity is unknown
- **THEN** the TUI SHALL display known token usage values and clearly report that remaining capacity is unavailable

### Requirement: Context usage updates after conversation changes
The context visualization SHALL reflect the current session state when invoked.

#### Scenario: Usage changes after assistant turn
- **WHEN** a completed assistant turn updates token usage and the user runs `/context`
- **THEN** the displayed context usage SHALL include the latest available usage totals

#### Scenario: Tool results are shown separately
- **WHEN** the current conversation contains tool result messages and the user runs `/context`
- **THEN** the visualization SHALL include tool-result usage as a distinct segment or row

### Requirement: Context visualization remains readable in narrow terminals
The context visualization SHALL render a readable fallback when the terminal is too narrow for a full segmented bar.

#### Scenario: Narrow terminal
- **WHEN** the user runs `/context` in a narrow terminal
- **THEN** the TUI SHALL display the usage breakdown as aligned text rows without overlapping labels
