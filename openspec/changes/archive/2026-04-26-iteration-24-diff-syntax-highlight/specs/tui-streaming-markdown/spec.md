## MODIFIED Requirements

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
