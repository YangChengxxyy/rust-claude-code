## ADDED Requirements

### Requirement: Cancel active streaming output
The TUI SHALL allow the user to cancel the currently active streaming response without exiting the application.

#### Scenario: Cancel stream with Escape
- **WHEN** a response is actively streaming and the user presses `Escape`
- **THEN** the TUI SHALL stop the current stream and return control to the input state

#### Scenario: Ctrl+C cancels active stream
- **WHEN** a response is actively streaming and the user presses `Ctrl+C`
- **THEN** the TUI SHALL cancel the current stream instead of terminating the application

### Requirement: Ctrl+C exits only when idle
The TUI SHALL exit on `Ctrl+C` only when no streaming response is active and the input/editor state does not need cancellation semantics.

#### Scenario: Ctrl+C exits while idle
- **WHEN** no response is streaming and the input area is idle
- **THEN** pressing `Ctrl+C` SHALL terminate the TUI session

### Requirement: Clear screen shortcut
The TUI SHALL support `Ctrl+L` to clear the visible chat area while preserving the current session state and input buffer.

#### Scenario: Clear visible chat viewport
- **WHEN** the user presses `Ctrl+L`
- **THEN** the TUI SHALL clear or redraw the visible terminal chat area without deleting message history or the current input draft
