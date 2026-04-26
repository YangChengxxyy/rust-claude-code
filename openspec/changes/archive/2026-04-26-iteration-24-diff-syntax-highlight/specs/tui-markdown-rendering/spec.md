## MODIFIED Requirements

### Requirement: Code block rendering
The TUI SHALL render fenced code blocks as visually separated blocks with syntax highlighting applied when a recognized language is specified, and MUST preserve whitespace and line breaks inside the block.

#### Scenario: Render fenced code block with syntax highlighting
- **WHEN** an assistant message contains a triple-backtick fenced code block with a recognized language identifier (e.g., ` ```rust `)
- **THEN** the TUI SHALL display it as a separated block with language-specific syntax coloring applied to keywords, strings, comments, types, and other token categories

#### Scenario: Render code block without syntax highlighter support
- **WHEN** syntax highlighting is unavailable or unsupported for the code block language
- **THEN** the code block SHALL still be rendered with preserved formatting and a distinct visual container using `palette.text` color

#### Scenario: Render code block with no language tag
- **WHEN** a code block has no language identifier after the opening fence
- **THEN** the code block SHALL render with preserved formatting in `palette.text` color without syntax highlighting
