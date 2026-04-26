## Requirements

### Requirement: Incremental markdown rendering during streaming
The TUI SHALL render markdown formatting in real time as streaming text deltas arrive, rather than displaying raw markdown syntax until the stream completes.

#### Scenario: Heading formatted during streaming
- **WHEN** a `StreamDelta` containing `## Some Heading\n` arrives during active streaming
- **THEN** the heading SHALL be rendered with heading styling immediately, not as the raw text `## Some Heading`

#### Scenario: List items formatted during streaming
- **WHEN** a `StreamDelta` containing `- list item\n` or `1. list item\n` arrives during active streaming
- **THEN** the list item SHALL be rendered with proper indentation and list marker styling immediately

#### Scenario: Inline emphasis formatted during streaming
- **WHEN** a complete line containing `**bold**`, `*italic*`, or `` `code` `` is available in the streaming buffer
- **THEN** the inline spans SHALL be rendered with their respective emphasis styles

### Requirement: Code block formatted during streaming with syntax highlighting
When streaming deltas form a fenced code block, completed lines inside the code block SHALL be rendered with syntax highlighting (when a recognized language is specified), and the pending incomplete line SHALL render with plain text color.

#### Scenario: Code block formatted during streaming
- **WHEN** streaming deltas form a fenced code block (opening ` ``` `, content lines, closing ` ``` `) with a recognized language identifier
- **THEN** completed lines inside the code block SHALL be rendered with language-specific syntax coloring as soon as each line is complete, and the block SHALL close when the closing fence arrives

#### Scenario: Streaming code block pending line
- **WHEN** a streaming code block has a partial line (no trailing newline yet)
- **THEN** the pending line SHALL render in plain `palette.text` color without syntax highlighting

#### Scenario: Streaming highlight state continuity
- **WHEN** a code block spans multiple streaming deltas and each delta completes one or more lines
- **THEN** the syntax highlighter SHALL maintain parse state across lines so that multi-line constructs (e.g., multi-line strings, block comments) are highlighted correctly

### Requirement: Line-oriented incremental parser with state machine
The streaming markdown parser SHALL maintain a block-level state machine that tracks the current context (paragraph, code block, list) across delta boundaries, and SHALL process complete lines without re-parsing previously parsed lines.

#### Scenario: Code block state preserved across deltas
- **WHEN** a delta contains an opening code fence ` ```rust ` and subsequent deltas contain code lines without a closing fence
- **THEN** all code lines SHALL be recognized as code block content until a closing fence delta arrives

#### Scenario: Previously parsed lines not re-parsed
- **WHEN** 100 lines have been parsed and cached, and a new delta adds line 101
- **THEN** only line 101 (and the pending incomplete line) SHALL be parsed; the first 100 lines SHALL be served from cache

### Requirement: Pending line rendering
The parser SHALL maintain a pending line buffer for incomplete lines (no trailing newline received yet) and SHALL render the pending line with best-effort inline formatting on each frame.

#### Scenario: Partial line displayed during streaming
- **WHEN** a delta `"The quick brown"` arrives without a trailing newline
- **THEN** the text SHALL be displayed immediately as a pending line, and SHALL be re-parsed when the line completes with a subsequent newline

#### Scenario: Pending line merges with next delta
- **WHEN** pending line contains `"The quick"` and a new delta `" brown fox\n"` arrives
- **THEN** the pending line SHALL be cleared, the complete line `"The quick brown fox"` SHALL be parsed and added to the line cache
