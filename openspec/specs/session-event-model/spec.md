## Requirements

### Requirement: SessionEvent enum represents all JSONL log entry types
The system SHALL define a `SessionEvent` enum with serde internal tagging (`#[serde(tag = "type")]`) that covers all structured log entries in a session JSONL file.

#### Scenario: Header event as first line
- **WHEN** a new session JSONL file is created
- **THEN** the first line SHALL be a `Header` event containing `id`, `model`, `model_setting`, `cwd`, and `created_at` fields

#### Scenario: User message event
- **WHEN** a user message is appended to the session
- **THEN** a `UserMessage` event SHALL be written containing the full `Message` struct

#### Scenario: Assistant message event
- **WHEN** an assistant response is appended to the session
- **THEN** an `AssistantMessage` event SHALL be written containing the full `Message` struct including usage data

#### Scenario: Compact boundary event
- **WHEN** a compaction operation completes
- **THEN** a `CompactBoundary` event SHALL be written containing the compaction summary text

#### Scenario: Usage update event
- **WHEN** cumulative token usage is updated
- **THEN** a `UsageUpdate` event SHALL be written containing the full `Usage` struct

#### Scenario: Permission change event
- **WHEN** the permission mode or rules change during a session
- **THEN** a `PermissionChange` event SHALL be written containing the new `PermissionMode` and any updated allow/deny rules

#### Scenario: Session end event
- **WHEN** a session terminates normally
- **THEN** a `SessionEnd` event SHALL be written containing the `updated_at` timestamp

### Requirement: SessionEvent serializes with type tag
Each `SessionEvent` variant SHALL serialize to a JSON object with a `"type"` field identifying the variant (e.g., `"type": "header"`, `"type": "user_message"`, `"type": "session_end"`).

#### Scenario: Roundtrip serialization
- **WHEN** any `SessionEvent` variant is serialized to JSON and deserialized back
- **THEN** the result SHALL be equal to the original value

#### Scenario: Unknown type tolerance
- **WHEN** a JSONL line contains an unrecognized `"type"` value
- **THEN** the reader SHALL skip that line with a warning rather than failing the entire load
