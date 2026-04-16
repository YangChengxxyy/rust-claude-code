## ADDED Requirements

### Requirement: SystemBlock type for structured system prompt
The `api` crate SHALL define a `SystemBlock` struct with fields: `type: String` (always `"text"`), `text: String`, and `cache_control: Option<CacheControl>`. The `CacheControl` struct SHALL have a `type: String` field (always `"ephemeral"`). The `SystemPrompt` enum SHALL include a `StructuredBlocks(Vec<SystemBlock>)` variant that serializes as a JSON array of system blocks.

#### Scenario: SystemBlock serialization with cache_control
- **WHEN** a `SystemBlock` is created with `text = "You are helpful"` and `cache_control = Some(CacheControl { type: "ephemeral" })`
- **THEN** it SHALL serialize to `{"type":"text","text":"You are helpful","cache_control":{"type":"ephemeral"}}`

#### Scenario: SystemBlock serialization without cache_control
- **WHEN** a `SystemBlock` is created with `text = "You are helpful"` and `cache_control = None`
- **THEN** it SHALL serialize to `{"type":"text","text":"You are helpful"}` with no `cache_control` field present

#### Scenario: SystemPrompt StructuredBlocks serialization
- **WHEN** a `SystemPrompt::StructuredBlocks` is created with two `SystemBlock` entries, the last one having `cache_control`
- **THEN** it SHALL serialize as a JSON array with two objects, only the last having `cache_control`

### Requirement: System prompt cache_control injection
When building an API request, the system prompt SHALL be converted to `SystemPrompt::StructuredBlocks` format with `cache_control: { type: "ephemeral" }` on the last system block. This enables Anthropic's prompt caching for the system prompt prefix.

#### Scenario: Single system prompt text with cache_control
- **WHEN** `build_request()` constructs a request with a system prompt string of "You are a helpful assistant."
- **THEN** the serialized request SHALL contain `"system":[{"type":"text","text":"You are a helpful assistant.","cache_control":{"type":"ephemeral"}}]`

#### Scenario: Multi-block system prompt with cache_control on last block
- **WHEN** the system prompt is composed of multiple sections (core prompt, tool descriptions, environment, CLAUDE.md)
- **THEN** `cache_control` SHALL be set only on the last system block, maximizing the cacheable prefix length

### Requirement: Message-level cache_control injection
When building an API request, the system SHALL inject `cache_control: { type: "ephemeral" }` on the last content block of the last message in the messages array. This is performed during request serialization without modifying the core `ContentBlock` types.

#### Scenario: Cache control on last message's last content block
- **WHEN** `build_request()` serializes a conversation with 5 messages, the last being a user message with a single text block
- **THEN** that text block in the serialized JSON SHALL include `"cache_control":{"type":"ephemeral"}`

#### Scenario: Cache control only on last message
- **WHEN** a conversation has 5 messages
- **THEN** only the last message's last content block SHALL have `cache_control`; all other messages' blocks SHALL NOT have `cache_control`

#### Scenario: Cache control with multi-block message
- **WHEN** the last message has multiple content blocks (e.g., multiple `tool_result` blocks)
- **THEN** `cache_control` SHALL be on the very last content block only

### Requirement: Cache hit visibility in TUI
The TUI status bar SHALL display cache hit information showing the ratio of `cache_read_input_tokens` to total input tokens from the most recent API response.

#### Scenario: Cache hit display after first request
- **WHEN** the first API response returns `cache_read_input_tokens: 0` and `input_tokens: 5000`
- **THEN** the status bar SHALL show cache information indicating 0% cache hit

#### Scenario: Cache hit display after subsequent request
- **WHEN** a subsequent API response returns `cache_read_input_tokens: 4500` and `input_tokens: 5000`
- **THEN** the status bar SHALL show cache information indicating ~90% cache hit
