## ADDED Requirements

### Requirement: Shared messages support image content blocks
The shared message model SHALL represent image content blocks as first-class content alongside text, tool, tool result, and thinking blocks.

#### Scenario: Deserialize image content block
- **WHEN** the system receives or loads a message containing an image block
- **THEN** it parses that block into a typed shared content representation without collapsing it into unknown JSON

#### Scenario: Serialize image content block
- **WHEN** the system persists or forwards a message containing an image block
- **THEN** it serializes the block using the same shared representation so the content can round-trip safely

### Requirement: Image content blocks survive session persistence
The system SHALL preserve image content blocks in saved session files.

#### Scenario: Save and reload session with image content
- **WHEN** a session containing image content blocks is written to disk and later reloaded
- **THEN** the reloaded session preserves the image blocks and their metadata
