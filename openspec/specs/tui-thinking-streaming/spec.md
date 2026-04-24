## ADDED Requirements

### Requirement: Real-time thinking content display during streaming
The TUI SHALL display thinking/reasoning content in real time as `ThinkingDelta` events arrive, rather than hiding the content behind a static spinner.

#### Scenario: Thinking text appears incrementally
- **WHEN** `ThinkingStart` is received followed by `ThinkingDelta("Let me analyze")` and then `ThinkingDelta(" this code...")`
- **THEN** the thinking area SHALL display `"Let me analyze"` after the first delta and `"Let me analyze this code..."` after the second delta

#### Scenario: Thinking content visually distinct from response text
- **WHEN** thinking content is being streamed
- **THEN** it SHALL be rendered with a visually distinct style (dim/italic) and preceded by a `[Thinking]` header to separate it from the assistant's response text

### Requirement: Thinking block fold/unfold during streaming
The TUI SHALL allow the user to collapse (fold) the live thinking content during streaming so that only the header line is visible, and expand (unfold) it to see the full content.

#### Scenario: Collapse thinking during streaming
- **WHEN** thinking content is streaming and the user presses the fold key (Tab)
- **THEN** the thinking area SHALL collapse to show only the `[Thinking]` header with a progress indicator, and new thinking deltas SHALL continue accumulating but not be visible

#### Scenario: Expand thinking during streaming
- **WHEN** thinking content is collapsed and the user presses the unfold key (Tab)
- **THEN** the full accumulated thinking content SHALL be displayed and new deltas SHALL appear in real time

### Requirement: Thinking block finalization
When the thinking block completes, the TUI SHALL convert the streaming thinking display into a finalized collapsible `ChatMessage::Thinking` entry consistent with the existing rendering.

#### Scenario: Thinking block completes
- **WHEN** `ThinkingComplete` event is received (or `StreamEnd` with accumulated thinking content)
- **THEN** the streaming thinking display SHALL be replaced by a standard `ChatMessage::Thinking` message with summary and fold/unfold support

#### Scenario: Thinking cancelled mid-stream
- **WHEN** the user cancels the stream while thinking content is being streamed
- **THEN** the accumulated thinking content SHALL be converted to a `ChatMessage::Thinking` entry with a "(cancelled)" annotation in the summary
