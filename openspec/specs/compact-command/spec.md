## ADDED Requirements

### Requirement: /compact slash command
The TUI SHALL support a `/compact` slash command that manually triggers conversation compaction regardless of the current token count.

#### Scenario: User triggers /compact with sufficient history
- **WHEN** the user types `/compact` and the conversation has more than 4 messages
- **THEN** the system SHALL trigger compaction and display a status message indicating compaction is in progress

#### Scenario: User triggers /compact with insufficient history
- **WHEN** the user types `/compact` and the conversation has 4 or fewer messages
- **THEN** the system SHALL display a message indicating there is not enough history to compact

#### Scenario: /compact in help listing
- **WHEN** the user types `/help`
- **THEN** the help output SHALL include `/compact` with a brief description of its function

### Requirement: Compaction status feedback in TUI
The TUI SHALL display visual feedback during compaction operations, including a status message when compaction starts and a result message when compaction completes.

#### Scenario: Compaction in-progress feedback
- **WHEN** compaction is triggered (manually or automatically)
- **THEN** the TUI SHALL display a system message such as "Compacting conversation history..."

#### Scenario: Compaction completion feedback
- **WHEN** compaction completes successfully
- **THEN** the TUI SHALL display a system message with the compaction result summary (e.g., "Compacted 12 messages into summary. Preserved 8 recent messages. Estimated tokens: 170K -> 85K")

#### Scenario: Compaction failure feedback
- **WHEN** compaction fails (e.g., API error)
- **THEN** the TUI SHALL display an error message describing the failure reason

### Requirement: TUI AppEvent for compaction
The `AppEvent` enum SHALL include compaction-related variants to communicate compaction status from the worker to the TUI rendering layer.

#### Scenario: CompactionStart event
- **WHEN** compaction begins
- **THEN** a `CompactionStart` event SHALL be sent through the TUI bridge

#### Scenario: CompactionComplete event
- **WHEN** compaction finishes successfully
- **THEN** a `CompactionComplete` event containing the `CompactionResult` SHALL be sent through the TUI bridge

#### Scenario: CompactionError event
- **WHEN** compaction fails
- **THEN** an `Error` event with the compaction error description SHALL be sent through the TUI bridge

### Requirement: /compact command routing through worker
The `/compact` command SHALL be routed from the TUI to the background worker task for execution. The worker SHALL distinguish a compact request from a normal user prompt.

#### Scenario: Compact request sent to worker
- **WHEN** the user types `/compact`
- **THEN** the TUI SHALL send a special marker message (e.g., a string starting with `[COMPACT_REQUEST]`) through the existing user input channel to the worker

#### Scenario: Worker handles compact request
- **WHEN** the worker receives a message starting with `[COMPACT_REQUEST]`
- **THEN** the worker SHALL call `CompactionService::compact()` instead of `QueryLoop::run()`, and send appropriate compaction events through the TUI bridge

### Requirement: Session persistence with compacted history
Compacted conversation history SHALL be correctly saved to and restored from session files. The summary message SHALL be persisted as a regular `Message::user` with the `[COMPACTED]` prefix text.

#### Scenario: Save session after compaction
- **WHEN** compaction completes and the session is saved
- **THEN** the `SessionFile.messages` SHALL contain the compacted message list (summary + preserved messages)

#### Scenario: Restore session with compacted history
- **WHEN** a session containing a `[COMPACTED]` summary message is loaded via `--continue`
- **THEN** the restored `AppState.messages` SHALL include the summary message, and the QueryLoop SHALL be able to continue the conversation normally

### Requirement: Non-interactive mode compaction
In non-interactive (print) mode, auto-compaction SHALL still trigger if the token threshold is exceeded during multi-turn agent execution.

#### Scenario: Auto-compaction in print mode
- **WHEN** running in `--print` mode with `--max-turns 20` and the conversation exceeds the token threshold during tool-use loops
- **THEN** auto-compaction SHALL trigger transparently, and the final output SHALL reflect the continued conversation after compaction

### Requirement: /compact retention strategy argument
The TUI SHALL support optional retention strategy arguments for `/compact` while preserving the existing no-argument behavior.

#### Scenario: Compact with default strategy
- **WHEN** the user types `/compact` with no additional arguments
- **THEN** the system SHALL use the existing default compaction strategy

#### Scenario: Compact with named strategy
- **WHEN** the user types `/compact aggressive` or `/compact preserve-recent`
- **THEN** the system SHALL trigger compaction using the selected named retention strategy

#### Scenario: Compact with unknown strategy
- **WHEN** the user types `/compact` with an unrecognized strategy name
- **THEN** the system SHALL display a user-facing error and SHALL NOT start compaction

### Requirement: /compact help describes strategies
The `/help` output SHALL describe supported `/compact` retention strategy arguments.

#### Scenario: Help includes compact strategies
- **WHEN** the user types `/help`
- **THEN** the help output SHALL include `/compact [default|aggressive|preserve-recent]` or equivalent strategy documentation
