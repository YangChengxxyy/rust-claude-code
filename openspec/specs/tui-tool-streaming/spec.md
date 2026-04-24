## ADDED Requirements

### Requirement: Tool input delta forwarding
The query loop SHALL forward `InputJsonDelta` events from the API stream through the TUI bridge as `ToolInputDelta` events so the TUI can display tool call construction in progress.

#### Scenario: Input JSON delta forwarded to TUI
- **WHEN** the API stream emits an `InputJsonDelta` with a JSON fragment for a tool call
- **THEN** the query loop SHALL send a `ToolInputDelta` event through the bridge containing the tool name and the JSON fragment

#### Scenario: Tool name available at streaming start
- **WHEN** a `ContentBlockStart` for a `tool_use` block is received (containing the tool name)
- **THEN** the bridge SHALL receive a `ToolInputStreamStart { name }` event before any `ToolInputDelta` events for that tool

### Requirement: Incremental tool call display
The TUI SHALL display a tool call card showing the tool name and the accumulated partial JSON input as tool input deltas arrive.

#### Scenario: Tool call card appears on stream start
- **WHEN** `ToolInputStreamStart { name: "Bash" }` is received
- **THEN** the TUI SHALL display a tool call card with the header "Bash" and a "constructing..." indicator

#### Scenario: Partial JSON displayed incrementally
- **WHEN** `ToolInputDelta` events arrive with fragments `{"comma`, `nd": "git st`, `atus"}`
- **THEN** the tool call card SHALL display the accumulated JSON `{"command": "git status"}` progressively as each fragment arrives

#### Scenario: Tool call card transitions to finalized tool use
- **WHEN** the tool use block completes and `ToolUseStart` is received with the full input
- **THEN** the streaming tool call card SHALL be replaced by the standard `ChatMessage::ToolUse` display with the complete input summary

### Requirement: Multiple sequential tool calls
The TUI SHALL handle streaming of multiple tool calls in a single assistant turn, displaying each tool's construction and completion in sequence.

#### Scenario: Second tool streams after first completes
- **WHEN** tool A finishes streaming its input and tool B starts streaming
- **THEN** the streaming tool card for tool A SHALL be finalized and tool B's streaming card SHALL appear below it
