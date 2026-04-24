## MODIFIED Requirements

### Requirement: Cancel active streaming output
The TUI SHALL allow the user to cancel the currently active streaming response without exiting the application. Cancellation SHALL apply to all streaming phases including text streaming, thinking streaming, and tool input streaming.

#### Scenario: Cancel stream with Escape
- **WHEN** a response is actively streaming and the user presses `Escape`
- **THEN** the TUI SHALL stop the current stream and return control to the input state

#### Scenario: Ctrl+C cancels active stream
- **WHEN** a response is actively streaming and the user presses `Ctrl+C`
- **THEN** the TUI SHALL cancel the current stream instead of terminating the application

#### Scenario: Cancel during thinking streaming
- **WHEN** thinking content is actively streaming and the user presses `Escape` or `Ctrl+C`
- **THEN** the TUI SHALL cancel the stream, finalize accumulated thinking content as a `ChatMessage::Thinking` with a "(cancelled)" annotation, and return control to the input state

#### Scenario: Cancel during tool input streaming
- **WHEN** tool input JSON is actively streaming and the user presses `Escape` or `Ctrl+C`
- **THEN** the TUI SHALL cancel the stream, discard the incomplete tool input display, and return control to the input state

#### Scenario: Terminal state clean after cancel
- **WHEN** a stream is cancelled during any streaming phase
- **THEN** the terminal SHALL be in a clean state with no orphaned streaming buffers, the input area SHALL be active, and no visual artifacts SHALL remain from the cancelled stream
