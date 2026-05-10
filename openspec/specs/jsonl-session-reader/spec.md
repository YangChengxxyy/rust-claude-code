## Requirements

### Requirement: SessionReader loads JSONL sessions
The `SessionReader` SHALL parse a `.jsonl` file line-by-line, reconstructing a `SessionFile` from the sequence of `SessionEvent` entries.

#### Scenario: Load a complete JSONL session
- **WHEN** `load_from_jsonl(path)` is called on a valid `.jsonl` file
- **THEN** it SHALL return a `SessionFile` with all messages, metadata, and usage reconstructed from the event log

#### Scenario: Header event populates session metadata
- **WHEN** the first line is a `Header` event
- **THEN** the returned `SessionFile` SHALL have `id`, `model`, `model_setting`, `cwd`, and `created_at` populated from the header

#### Scenario: Messages reconstructed in order
- **WHEN** the JSONL contains `UserMessage` and `AssistantMessage` events
- **THEN** the returned `SessionFile.messages` SHALL contain all messages in the order they appear in the file

#### Scenario: CompactBoundary resets message history
- **WHEN** a `CompactBoundary` event is encountered during loading
- **THEN** messages before the boundary SHALL be replaced by a single summary message, matching the behavior of in-memory compaction

#### Scenario: UsageUpdate replaces cumulative usage
- **WHEN** a `UsageUpdate` event is encountered
- **THEN** the returned `SessionFile.total_usage` SHALL reflect the usage from the most recent `UsageUpdate` event

#### Scenario: PermissionChange updates permission state
- **WHEN** a `PermissionChange` event is encountered
- **THEN** the returned `SessionFile.permission_mode` and rules SHALL reflect the most recent `PermissionChange` event

#### Scenario: Truncated last line is skipped
- **WHEN** the last line of the JSONL file is invalid JSON (e.g., truncated by crash)
- **THEN** the reader SHALL skip that line and return the session reconstructed from all valid preceding lines

### Requirement: SessionReader loads legacy JSON sessions
The `SessionReader` SHALL support loading existing `.json` session files using the current `serde_json::from_str` deserialization.

#### Scenario: Load a legacy JSON session
- **WHEN** `load_from_json(path)` is called on a valid `.json` file
- **THEN** it SHALL return a `SessionFile` identical to the current `SessionFile::load()` behavior

### Requirement: SessionReader auto-detects format
The `SessionReader` SHALL provide a `load(path)` method that auto-detects whether the file is JSONL or legacy JSON based on file extension.

#### Scenario: Auto-detect JSONL by extension
- **WHEN** `load(path)` is called with a `.jsonl` file
- **THEN** it SHALL use the JSONL parser

#### Scenario: Auto-detect JSON by extension
- **WHEN** `load(path)` is called with a `.json` file
- **THEN** it SHALL use the legacy JSON parser

#### Scenario: No extension fallback
- **WHEN** `load(path)` is called with a file that has neither `.json` nor `.jsonl` extension
- **THEN** it SHALL attempt JSONL parsing first, falling back to JSON parsing if the first attempt fails
