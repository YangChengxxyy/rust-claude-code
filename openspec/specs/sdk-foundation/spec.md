## Purpose

Define the SDK crate foundation including output traits, permission/UI traits, QueryLoop refactoring, SessionBuilder, and TuiBridge trait implementations.

## Requirements

### Requirement: SDK crate exists with zero TUI/CLI dependencies
The project workspace SHALL include a `sdk` crate at `crates/sdk/` that compiles independently without any dependency on `rust-claude-tui` or `rust-claude-cli`. The crate SHALL depend on `rust-claude-core`, `rust-claude-api`, `rust-claude-tools`, and `rust-claude-mcp`.

#### Scenario: SDK crate builds in isolation
- **WHEN** `cargo build -p rust-claude-sdk` is executed
- **THEN** the crate SHALL compile successfully without linking any TUI or CLI symbols

#### Scenario: SDK crate has no TUI types in its public API
- **WHEN** reviewing `crates/sdk/src/lib.rs` public exports
- **THEN** no type from `rust_claude_tui` SHALL appear in any public function signature, trait, or struct

### Requirement: OutputSink trait for streaming events
The SDK SHALL define an `OutputSink` trait that abstracts streaming text, thinking, tool input, tool results, usage, errors, and todo updates away from any specific UI implementation.

#### Scenario: OutputSink receives text delta
- **WHEN** the agent loop produces a text delta "hello"
- **THEN** the `OutputSink::text_delta("hello")` method SHALL be called

#### Scenario: OutputSink receives thinking delta
- **WHEN** the agent loop produces a thinking delta "let me consider..."
- **THEN** the `OutputSink::thinking_delta("let me consider...")` method SHALL be called

#### Scenario: OutputSink receives tool input streaming
- **WHEN** the agent loop receives partial JSON input for a tool named "Bash"
- **THEN** `OutputSink::tool_input_start("Bash")` SHALL be called, followed by `OutputSink::tool_input_delta("Bash", partial_json)` for each fragment

#### Scenario: No OutputSink provided (headless mode)
- **WHEN** a Session is built without an OutputSink
- **THEN** the agent loop SHALL execute normally without any output calls (no-op)

### Requirement: PermissionUI trait for interactive permission decisions
The SDK SHALL define a `PermissionUI` trait with an async `request` method that takes a tool name and input and returns a `PermissionDecision`.

#### Scenario: PermissionUI allows a tool
- **WHEN** the agent loop requests permission for tool "Bash" and the UI returns `PermissionDecision::Allow`
- **THEN** the tool SHALL execute normally

#### Scenario: PermissionUI denies a tool
- **WHEN** the agent loop requests permission for tool "Bash" and the UI returns `PermissionDecision::Deny`
- **THEN** the tool SHALL NOT execute and a `Permission denied by user` error SHALL be added as a tool result

#### Scenario: No PermissionUI provided
- **WHEN** a Session is built without a PermissionUI
- **THEN** all `NeedsConfirmation` permission checks SHALL be auto-denied

### Requirement: UserQuestionUI trait for structured user questions
The SDK SHALL define a `UserQuestionUI` trait with an async `ask` method that takes an `AskUserQuestionRequest` and returns an `Option<AskUserQuestionResponse>`.

#### Scenario: UserQuestionUI returns a response
- **WHEN** the AskUserQuestionTool calls the UI with a question and the user selects an option
- **THEN** the tool SHALL receive the selected response

#### Scenario: No UserQuestionUI provided
- **WHEN** a Session is built without a UserQuestionUI and AskUserQuestionTool is invoked
- **THEN** the tool SHALL return an error indicating user questions are not available

### Requirement: QueryLoop uses traits instead of TuiBridge
The `QueryLoop` struct SHALL replace its `bridge: Option<rust_claude_tui::TuiBridge>` field with three optional trait object fields: `output: Option<Box<dyn OutputSink>>`, `permission_ui: Option<Box<dyn PermissionUI>>`, and `user_question_ui: Option<Box<dyn UserQuestionUI>>`.

#### Scenario: QueryLoop streams text via OutputSink
- **WHEN** a streaming response produces text content
- **THEN** `self.output.as_ref().unwrap().text_delta(text)` SHALL be called instead of `self.bridge.as_ref().unwrap().send_stream_delta(text)`

#### Scenario: QueryLoop checks permission via PermissionUI
- **WHEN** a tool requires confirmation and PermissionUI is set
- **THEN** `self.permission_ui.as_ref().unwrap().request(tool_name, input)` SHALL be called

#### Scenario: QueryLoop asks user questions via UserQuestionUI
- **WHEN** AskUserQuestionTool executes and UserQuestionUI is set
- **THEN** `self.user_question_ui.as_ref().unwrap().ask(request)` SHALL be called

### Requirement: SessionBuilder constructs agent sessions
The SDK SHALL provide a `SessionBuilder` that accepts configuration and produces a `Session`. Required fields (client, tools) SHALL be enforced at compile time. Optional fields (output, permission_ui, user_question_ui, max_rounds, compaction_config, hook_runner) SHALL be set via builder methods.

#### Scenario: Session built with minimum configuration
- **WHEN** `Session::builder().client(client).tools(tools).build()` is called
- **THEN** a valid `Session` SHALL be returned

#### Scenario: Session built with full configuration
- **WHEN** `Session::builder()` is called with all optional methods chained
- **THEN** the resulting `Session` SHALL reflect all provided configurations

### Requirement: Session provides send and send_streaming methods
The SDK's `Session` type SHALL provide `send(prompt: String) -> Result<Message, Error>` for collected responses and `send_streaming(prompt: String) -> Result<EventStream, Error>` for real-time event streams.

#### Scenario: send returns complete message after tool loop
- **WHEN** `session.send("run ls")` is called and the agent uses Bash tool then responds with text
- **THEN** the method SHALL return the final assistant `Message` after all tool executions complete

#### Scenario: send_streaming yields events in real time
- **WHEN** `session.send_streaming("hello")` is called
- **THEN** the returned stream SHALL yield `SessionEvent::TextDelta` events as tokens arrive, followed by `SessionEvent::Complete` when the turn finishes

### Requirement: TuiBridge implements SDK traits
The existing `TuiBridge` in `crates/tui/` SHALL implement `OutputSink`, `PermissionUI`, and `UserQuestionUI` traits from the SDK crate.

#### Scenario: TuiBridge as OutputSink
- **WHEN** `OutputSink::text_delta()` is called on a TuiBridge
- **THEN** the bridge SHALL send an `AppEvent::StreamDelta` through its event channel

#### Scenario: TuiBridge as PermissionUI
- **WHEN** `PermissionUI::request()` is called on a TuiBridge
- **THEN** the bridge SHALL send an `AppEvent::PermissionRequest` with a oneshot channel and await the response

#### Scenario: TuiBridge as UserQuestionUI
- **WHEN** `UserQuestionUI::ask()` is called on a TuiBridge
- **THEN** the bridge SHALL send an `AppEvent::UserQuestionRequest` with a oneshot channel and await the response
