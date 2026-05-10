## ADDED Requirements

### Requirement: Tools declare interrupt behavior
Each tool SHALL expose interrupt behavior metadata that tells the executor whether user interruption cancels the tool or waits for it to finish.

#### Scenario: Default tool behavior is cancelable
- **WHEN** a tool does not override its interrupt behavior
- **THEN** the system treats the tool as cancelable on user interrupt

#### Scenario: Blocking tools are not cancelled by user interrupt
- **WHEN** a tool declares blocking interrupt behavior
- **THEN** the system waits for the tool to finish during turn discard instead of canceling it

### Requirement: Bash sibling failure cancellation
The system SHALL cancel other in-flight Bash tools from the same assistant turn when one Bash tool fails during streaming execution.

#### Scenario: Bash failure cancels sibling Bash commands
- **WHEN** multiple Bash tools from the same assistant turn are running and one Bash tool fails
- **THEN** the system cancels the other in-flight Bash tools from that assistant turn

#### Scenario: Bash failure does not cancel non-Bash tools
- **WHEN** a Bash tool fails while a FileRead tool from the same assistant turn is running
- **THEN** the system does not cancel the FileRead tool solely because of the Bash failure

### Requirement: Cancellation is scoped to the current assistant turn
Tool cancellation caused by interruption or Bash sibling failure SHALL apply only to tools scheduled for the current assistant turn.

#### Scenario: Later turns are unaffected by prior cancellation
- **WHEN** a streaming tool execution turn is canceled and a later assistant turn starts
- **THEN** the later turn receives fresh cancellation state and can execute tools normally
