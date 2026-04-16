## MODIFIED Requirements

### Requirement: TUI thinking block display
The TUI SHALL display thinking blocks with a distinct visual treatment: during streaming, show a spinner with "Thinking..."; after completion, show a collapsed summary line indicating the thinking duration or token count. The UI SHALL also support explicitly expanding and collapsing a completed thinking block for inspection.

#### Scenario: Thinking block during streaming
- **WHEN** the stream contains `ThinkingDelta` events
- **THEN** the TUI SHALL display a spinner or animated indicator with "Thinking..."

#### Scenario: Thinking block after completion
- **WHEN** a thinking block completes (ContentBlockStop received)
- **THEN** the TUI SHALL display a collapsed summary (e.g., "Thought for N tokens") instead of the full thinking text

#### Scenario: Expand completed thinking block
- **WHEN** the user focuses a completed thinking block summary and triggers the expand action
- **THEN** the TUI SHALL reveal the full thinking content for that block within the message view

#### Scenario: Collapse expanded thinking block
- **WHEN** the user triggers the expand/collapse action on an already expanded thinking block
- **THEN** the TUI SHALL collapse the block back to its summary representation
