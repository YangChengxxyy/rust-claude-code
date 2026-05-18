## MODIFIED Requirements

### Requirement: SDK crate exists with zero TUI/CLI dependencies
The project workspace SHALL include a `sdk` crate at `crates/sdk/` that compiles independently without any dependency on `rust-claude-tui` or `rust-claude-cli`. The crate SHALL depend on shared lower-level crates only and SHALL expose stable public types from `sdk/src/lib.rs`.

#### Scenario: SDK crate builds in isolation
- **WHEN** `cargo build -p rust-claude-sdk` is executed
- **THEN** the crate SHALL compile successfully without linking any TUI or CLI symbols

#### Scenario: SDK crate has no TUI types in its public API
- **WHEN** reviewing `crates/sdk/src/lib.rs` public exports
- **THEN** no type from `rust_claude_tui` SHALL appear in any public function signature, trait, or struct

#### Scenario: SDK exports stable entry points
- **WHEN** reviewing `crates/sdk/src/lib.rs` public exports
- **THEN** `Session`, `SessionBuilder`, `ResponseStream`, `ResponseEvent`, and SDK error/result aliases SHALL be exported as the primary public API

### Requirement: SessionBuilder constructs agent sessions
The SDK SHALL provide `Session::builder() -> SessionBuilder` and a `SessionBuilder` that accepts configuration and produces a `Session`. Required runtime components SHALL either be supplied explicitly or derived from builder-provided configuration, and optional fields SHALL be set via builder methods.

#### Scenario: Session built with minimum configuration
- **WHEN** `Session::builder().api_key("key").build()` is called
- **THEN** a valid headless `Session` SHALL be returned with default model, default tools, default permission behavior, and no TUI/CLI dependencies

#### Scenario: Session built with full configuration
- **WHEN** `Session::builder()` is called with model, base URL, system prompt, permission mode, max rounds, output sink, permission UI, user question UI, hooks, and compaction configuration
- **THEN** the resulting `Session` SHALL reflect all provided configurations

#### Scenario: Session built from low-level components
- **WHEN** `Session::builder()` is given an explicit model client and tool registry
- **THEN** the resulting `Session` SHALL use those components instead of constructing defaults

### Requirement: Session provides send and send_streaming methods
The SDK's `Session` type SHALL provide `send(prompt: &str) -> Result<Message, Error>`, `send_with_tools(prompt: &str, tool_results: Vec<ToolResult>) -> Result<Message, Error>`, and `send_streaming(prompt: &str) -> Result<ResponseStream, Error>` for high-level agent interaction.

#### Scenario: send returns complete message after tool loop
- **WHEN** `session.send("run ls")` is called and the agent uses Bash tool then responds with text
- **THEN** the method SHALL return the final assistant `Message` after all tool executions complete

#### Scenario: send_with_tools accepts external tool results
- **WHEN** `session.send_with_tools(prompt, tool_results)` is called
- **THEN** the supplied tool results SHALL be incorporated into the session turn before the agent continues

#### Scenario: send_streaming yields events in real time
- **WHEN** `session.send_streaming("hello")` is called
- **THEN** the returned stream SHALL yield `ResponseEvent::TextDelta` events as tokens arrive, followed by `ResponseEvent::Done` when the turn finishes

## ADDED Requirements

### Requirement: SDK ResponseStream yields stable response events
The SDK SHALL provide a `ResponseStream` that implements `Stream<Item = ResponseEvent>` and converts internal agent loop callbacks into stable event values suitable for embedded callers.

#### Scenario: Text delta event is emitted
- **WHEN** the model streams text content
- **THEN** `ResponseStream` SHALL yield `ResponseEvent::TextDelta(String)` with the streamed text

#### Scenario: Thinking delta event is emitted
- **WHEN** the model streams thinking content
- **THEN** `ResponseStream` SHALL yield `ResponseEvent::ThinkingDelta(String)` with the streamed thinking text

#### Scenario: Tool events are emitted
- **WHEN** the agent starts a tool call and later receives its result
- **THEN** `ResponseStream` SHALL yield tool-use and tool-result events containing the tool id/name/input/result data

#### Scenario: Usage and completion events are emitted
- **WHEN** a turn receives usage data and completes
- **THEN** `ResponseStream` SHALL yield `ResponseEvent::Usage(Usage)` and then `ResponseEvent::Done`

### Requirement: SDK supports custom tool injection
The SDK SHALL allow callers to register custom tools through `SessionBuilder` before building a session.

#### Scenario: Custom tool is registered
- **WHEN** a caller builds a session with `SessionBuilder::with_tool(Box<dyn Tool>)`
- **THEN** the tool SHALL be present in the session tool registry and available to the agent loop

#### Scenario: Custom tool conflicts with existing tool
- **WHEN** a caller registers a custom tool whose name conflicts with an existing registered tool
- **THEN** session build SHALL return a clear error rather than silently replacing an existing tool

### Requirement: SDK example compiles independently
The SDK SHALL include a minimal example demonstrating programmatic usage without TUI or CLI dependencies.

#### Scenario: sdk_basic example builds
- **WHEN** `cargo build -p rust-claude-sdk --example sdk_basic` is executed
- **THEN** the example SHALL compile successfully

#### Scenario: sdk_basic demonstrates send
- **WHEN** a developer reads the `sdk_basic` example
- **THEN** it SHALL show constructing a session with `Session::builder()`, sending a prompt, and consuming the response or response stream
