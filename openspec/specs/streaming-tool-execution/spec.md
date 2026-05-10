## ADDED Requirements

### Requirement: Execute completed streamed tool blocks early
The system SHALL begin executing an approved tool-use block during assistant response streaming after that block has been fully received and before the assistant response has necessarily reached `message_stop`.

#### Scenario: First tool starts before message stop
- **WHEN** a streamed assistant response emits a complete FileRead tool-use block and later emits additional content before `message_stop`
- **THEN** the FileRead execution starts before `message_stop` is processed

#### Scenario: Incomplete tool input is not executed
- **WHEN** a streamed tool-use block has not yet emitted a complete input JSON object
- **THEN** the system MUST NOT execute that tool

### Requirement: Preserve model-visible result order
The system SHALL return tool results to the next model request in the same order as the corresponding tool-use blocks appeared in the assistant response, regardless of execution completion order.

#### Scenario: Faster later tool does not reorder results
- **WHEN** two concurrency-safe tools are streamed in order and the second tool completes first
- **THEN** the tool results are appended in the first-tool then second-tool order

### Requirement: Respect tool concurrency safety
The system SHALL execute concurrency-safe tools concurrently while ensuring tools that are not concurrency-safe run serially in their original relative order.

#### Scenario: Concurrent-safe tools overlap
- **WHEN** multiple concurrency-safe tools are completed during the same streamed assistant turn
- **THEN** the system may execute them in parallel

#### Scenario: Non-concurrency-safe tools are serialized
- **WHEN** multiple non-concurrency-safe tools are completed during the same streamed assistant turn
- **THEN** the system executes the later non-concurrency-safe tool only after the previous non-concurrency-safe tool has finished

### Requirement: Reuse permission and hook behavior
The system SHALL apply the same permission checks and hook execution behavior for streamed tool execution as for collect-then-execute tool execution.

#### Scenario: Permission denial returns tool result
- **WHEN** a streamed tool-use block requires permission and permission is denied
- **THEN** the system returns a denied tool result without executing the tool implementation

#### Scenario: Hooks wrap streamed execution
- **WHEN** hooks are configured for a tool that is executed during streaming
- **THEN** the applicable hooks run with the same event semantics as the existing tool execution path

### Requirement: Fallback to existing execution path
The system SHALL fall back to the existing collect-then-execute path when streaming tool execution cannot safely complete and the full assistant message is still available.

#### Scenario: Streaming executor setup fails before committed results
- **WHEN** the streaming executor fails before any tool result has been committed to the next request and the assistant message can be reconstructed
- **THEN** the system discards streaming execution and executes tools through the existing collect-then-execute path

#### Scenario: Unrecoverable streaming failure is surfaced
- **WHEN** streaming execution fails and the assistant message cannot be reconstructed safely
- **THEN** the system reports the underlying failure instead of risking duplicate or partial tool execution

### Requirement: Discard pending streaming work on user interrupt
The system SHALL discard pending streamed tool execution on user interrupt and cancel interruptible in-flight tools for the current assistant turn.

#### Scenario: Interrupt cancels pending tools
- **WHEN** the user interrupts a streamed assistant turn while interruptible tools are pending or running
- **THEN** the system cancels those tools and does not append their results to a follow-up model request

#### Scenario: Blocking tool finishes before discard completes
- **WHEN** the user interrupts while a tool marked as blocking on interrupt is running
- **THEN** the system waits for that tool to finish before completing discard
