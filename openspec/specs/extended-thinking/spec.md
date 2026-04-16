## ADDED Requirements

### Requirement: ThinkingConfig type
The system SHALL define a `ThinkingConfig` enum in the `core` crate with three variants: `Disabled`, `Enabled { budget_tokens: u32 }`, and `Adaptive`. This type SHALL be serializable to the Anthropic API format.

#### Scenario: ThinkingConfig::Adaptive serialization
- **WHEN** `ThinkingConfig::Adaptive` is serialized for the API request
- **THEN** the output SHALL be `{"type":"enabled","budgetTokens":...}` where budgetTokens is determined by the model's max thinking budget, OR `{"type":"adaptive"}` if the API supports adaptive mode

#### Scenario: ThinkingConfig::Enabled serialization
- **WHEN** `ThinkingConfig::Enabled { budget_tokens: 10000 }` is serialized
- **THEN** the output SHALL be `{"type":"enabled","budget_tokens":10000}`

#### Scenario: ThinkingConfig::Disabled means no thinking field
- **WHEN** `ThinkingConfig::Disabled` is used in a request
- **THEN** the `thinking` field SHALL be omitted from the serialized API request

### Requirement: Thinking configuration in API request
The `CreateMessageRequest` SHALL include an optional `thinking` field. When present and not `Disabled`, it SHALL be serialized as part of the API request body. The `max_tokens` value MUST always be greater than `budget_tokens` when thinking is enabled.

#### Scenario: Request with adaptive thinking
- **WHEN** a request is built with `ThinkingConfig::Adaptive` and `max_tokens: 16384`
- **THEN** the serialized request body SHALL include a `thinking` field with type `"enabled"` and an appropriate `budget_tokens` value less than 16384

#### Scenario: Request without thinking
- **WHEN** a request is built with `ThinkingConfig::Disabled`
- **THEN** the serialized request body SHALL NOT contain a `thinking` field

### Requirement: Auto-detect thinking mode by model
The `QueryLoop` SHALL automatically determine the thinking configuration based on the model being used:
- Models matching `opus-4-6` or `sonnet-4-6` (including suffixes like `[1m]`) SHALL use `Adaptive` mode
- Other Claude 4+ models SHALL use `Enabled` with a model-appropriate budget
- Claude 3.x models or unknown models SHALL use `Disabled`

#### Scenario: Opus 4.6 uses adaptive thinking
- **WHEN** the runtime model is `claude-opus-4-6` or `claude-opus-4-6-20250514`
- **THEN** the thinking config SHALL be `Adaptive`

#### Scenario: Sonnet 4.6 uses adaptive thinking
- **WHEN** the runtime model is `claude-sonnet-4-6` or `claude-sonnet-4-6-20250514`
- **THEN** the thinking config SHALL be `Adaptive`

#### Scenario: Claude 3.5 disables thinking
- **WHEN** the runtime model is `claude-3-5-sonnet-20241022`
- **THEN** the thinking config SHALL be `Disabled`

### Requirement: Thinking block signature preservation
The `ContentBlock::Thinking` variant SHALL include an optional `signature: Option<String>` field. When the API returns a thinking block with a `signature`, it MUST be preserved in the message history and re-sent in subsequent API requests.

#### Scenario: Thinking block with signature deserialization
- **WHEN** the API returns `{"type":"thinking","thinking":"some reasoning","signature":"sig_abc123"}`
- **THEN** the deserialized `ContentBlock::Thinking` SHALL have `signature = Some("sig_abc123")`

#### Scenario: Thinking block without signature (backward compatibility)
- **WHEN** loading a legacy session file containing `{"type":"thinking","thinking":"old reasoning"}`
- **THEN** the deserialized `ContentBlock::Thinking` SHALL have `signature = None`

#### Scenario: Thinking block serialization preserves signature
- **WHEN** a `ContentBlock::Thinking` with `signature = Some("sig_abc")` is serialized for an API request
- **THEN** the serialized JSON SHALL include `"signature":"sig_abc"`

### Requirement: Thinking enabled by default
The `SessionSettings` SHALL include a `thinking_enabled: bool` field defaulting to `true`. Thinking SHALL be active unless explicitly disabled via CLI parameter `--no-thinking` or `thinking_enabled: false` in settings.

#### Scenario: Default session enables thinking
- **WHEN** a new session is created with default settings
- **THEN** `session.thinking_enabled` SHALL be `true`

#### Scenario: CLI --no-thinking disables thinking
- **WHEN** the CLI is invoked with `--no-thinking`
- **THEN** `session.thinking_enabled` SHALL be `false` and `ThinkingConfig::Disabled` SHALL be used

### Requirement: Forward-compatible content block deserialization
The `ContentBlock` enum SHALL handle unknown block types returned by the API without failing deserialization. Unknown types SHALL be represented as a catch-all variant that preserves the raw JSON.

#### Scenario: Unknown block type from API
- **WHEN** the API returns a content block with `{"type":"server_tool_use","id":"srvtool_1",...}`
- **THEN** deserialization SHALL succeed and the block SHALL be represented as an `Unknown` variant

#### Scenario: Known block types still parse correctly
- **WHEN** the API returns standard block types (`text`, `tool_use`, `tool_result`, `thinking`)
- **THEN** they SHALL deserialize to their specific typed variants as before

### Requirement: TUI thinking block display
The TUI SHALL display thinking blocks with a distinct visual treatment: during streaming, show a spinner with "Thinking..."; after completion, show a collapsed summary line indicating the thinking duration or token count. The UI SHALL also support explicitly expanding and collapsing a completed thinking block for inspection.

#### Scenario: Thinking block during streaming
- **WHEN** the stream contains `ThinkingDelta` events
- **THEN** the TUI SHALL display a spinner or animated indicator with "Thinking..."

#### Scenario: Thinking block after completion
- **WHEN** a thinking block completes (ContentBlockStop received)
- **THEN** the TUI SHALL display a collapsed summary (e.g., "Thought for N tokens") instead of the full thinking text

#### Scenario: Expand completed thinking block
- **WHEN** the user focuses a completed thinking block summary and triggers the expand action
- **THEN** the TUI SHALL reveal the full thinking content for that block within the message view

#### Scenario: Collapse expanded thinking block
- **WHEN** the user triggers the expand/collapse action on an already expanded thinking block
- **THEN** the TUI SHALL collapse the block back to its summary representation
