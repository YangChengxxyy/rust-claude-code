## ADDED Requirements

### Requirement: Keyboard scrolling for chat viewport
The TUI SHALL provide dedicated keyboard shortcuts for scrolling the chat content viewport independently from the input editor. `PageUp` MUST scroll the chat viewport toward older content, and `PageDown` MUST scroll it toward newer content.

#### Scenario: Scroll upward to older messages
- **WHEN** the chat content exceeds the visible viewport and the user presses `PageUp`
- **THEN** the TUI SHALL move the chat viewport upward to reveal older lines without modifying the current input buffer

#### Scenario: Scroll downward toward latest messages
- **WHEN** the user has previously scrolled upward and presses `PageDown`
- **THEN** the TUI SHALL move the chat viewport downward toward newer lines without submitting or editing the current input buffer

### Requirement: Jump to chat viewport boundaries
The TUI SHALL provide a direct way to jump to the oldest and newest positions of the chat viewport. `Ctrl+Home` MUST jump to the earliest visible position, and `Ctrl+End` MUST jump back to the latest visible position.

#### Scenario: Jump to oldest visible position
- **WHEN** the chat content exceeds the visible viewport and the user presses `Ctrl+Home`
- **THEN** the TUI SHALL move the viewport to the earliest available chat content position

#### Scenario: Jump back to latest output
- **WHEN** the user is reviewing older content and presses `Ctrl+End`
- **THEN** the TUI SHALL move the viewport to the bottom-most position where the newest chat content is visible

### Requirement: Auto-follow latest output only while at bottom
The TUI SHALL automatically keep the newest chat output visible only when the chat viewport is currently at the latest position. If the user has manually scrolled upward, newly appended content MUST NOT force the viewport back to the bottom.

#### Scenario: Auto-follow while already at bottom
- **WHEN** the chat viewport is at the latest position and new assistant, tool, or thinking content arrives
- **THEN** the TUI SHALL keep the newest content visible automatically

#### Scenario: Preserve historical view while reviewing older content
- **WHEN** the user has scrolled upward to review older chat content and new content arrives
- **THEN** the TUI SHALL preserve the current viewport position until the user explicitly scrolls back down or jumps to the bottom

### Requirement: Clamp chat scrolling to available content
The TUI SHALL clamp chat viewport scrolling within the available content range so that users cannot scroll past the oldest or newest renderable content.

#### Scenario: Attempt to scroll above oldest content
- **WHEN** the viewport is already at the earliest chat position and the user presses `PageUp`
- **THEN** the TUI SHALL keep the viewport at the earliest valid position

#### Scenario: Attempt to scroll below newest content
- **WHEN** the viewport is already at the newest chat position and the user presses `PageDown`
- **THEN** the TUI SHALL keep the viewport at the newest valid position
