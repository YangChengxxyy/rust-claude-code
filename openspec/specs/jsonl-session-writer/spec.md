## Requirements

### Requirement: SessionWriter appends events incrementally
The `SessionWriter` SHALL manage a JSONL file handle with `BufWriter` and append individual `SessionEvent` entries as single JSON lines.

#### Scenario: Creating a new session file
- **WHEN** a `SessionWriter` is created for a new session
- **THEN** it SHALL create the `.jsonl` file and write a `Header` event as the first line

#### Scenario: Appending a message
- **WHEN** `append_message(msg)` is called with a `Message`
- **THEN** the writer SHALL serialize the message as a `UserMessage` or `AssistantMessage` event (based on role) and append it as a single line to the JSONL file

#### Scenario: Appending a session event
- **WHEN** `append_event(event)` is called with a `SessionEvent`
- **THEN** the writer SHALL serialize the event and append it as a single line to the JSONL file

### Requirement: SessionWriter flushes after each agentic turn
The `SessionWriter` SHALL call `flush()` on the underlying `BufWriter` after each append operation to ensure data is written to disk.

#### Scenario: Flush on append_message
- **WHEN** `append_message()` completes
- **THEN** the buffered data SHALL be flushed to the OS

#### Scenario: Crash before explicit flush
- **WHEN** the process crashes after `append_message()` returns
- **THEN** the last appended message SHALL be recoverable from disk (because flush was called)

### Requirement: SessionWriter writes SessionEnd on normal termination
The `SessionWriter` SHALL provide a `finish()` method that appends a `SessionEnd` event and flushes.

#### Scenario: Normal session termination
- **WHEN** `finish()` is called
- **THEN** a `SessionEnd` event with the current timestamp SHALL be appended and flushed
- **AND** the file handle SHALL be closed

### Requirement: SessionWriter creates files in the sessions directory
The `SessionWriter` SHALL create JSONL files at `~/.config/rust-claude-code/sessions/{id}.jsonl`.

#### Scenario: Session file path
- **WHEN** a `SessionWriter` is created with session id "20260504_143022"
- **THEN** the file SHALL be created at `{sessions_dir}/20260504_143022.jsonl`
