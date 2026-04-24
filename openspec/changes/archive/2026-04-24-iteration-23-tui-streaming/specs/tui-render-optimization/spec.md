## ADDED Requirements

### Requirement: Frame rate limiting during streaming
The TUI SHALL limit rendering to approximately 30 frames per second during active streaming to batch rapid delta events and prevent flicker.

#### Scenario: Multiple deltas batched into single frame
- **WHEN** 5 `StreamDelta` events arrive within a 33ms window
- **THEN** the TUI SHALL render at most one frame for that window, reflecting all 5 deltas

#### Scenario: Immediate render when idle
- **WHEN** no streaming is active and a user input event occurs
- **THEN** the TUI SHALL render immediately without waiting for the next frame tick

### Requirement: Dirty-region tracking
The TUI SHALL track which areas of the display need redrawing and SHALL avoid redundant re-layout of unchanged regions during streaming.

#### Scenario: Only chat area redrawn on stream delta
- **WHEN** a `StreamDelta` event arrives and only the chat content has changed
- **THEN** the status bar and input area SHALL NOT be re-computed (their cached state SHALL be reused)

#### Scenario: Full redraw on resize
- **WHEN** a terminal resize event occurs
- **THEN** all regions SHALL be marked dirty and a full redraw SHALL occur

### Requirement: Sub-50ms single-frame render latency
The TUI SHALL render a single frame in less than 50ms even when the chat history contains 500+ tokens of accumulated streaming content.

#### Scenario: Large streaming buffer render performance
- **WHEN** 500 lines of markdown content have been accumulated in the streaming line cache
- **THEN** rendering the visible viewport (typically 30-50 lines) SHALL complete in under 50ms

### Requirement: Auto-scroll during streaming
The TUI SHALL automatically scroll to show the latest content during streaming, unless the user has manually scrolled up to review earlier content.

#### Scenario: Auto-scroll follows new content
- **WHEN** streaming is active and the user has not manually scrolled
- **THEN** the viewport SHALL automatically scroll to keep the newest content visible

#### Scenario: Manual scroll disables auto-scroll
- **WHEN** the user scrolls up during streaming
- **THEN** auto-scroll SHALL be disabled and the viewport SHALL stay at the user's scroll position

#### Scenario: Auto-scroll re-engages at bottom
- **WHEN** the user scrolls back to the bottom of the content during streaming
- **THEN** auto-scroll SHALL re-engage and follow new content
