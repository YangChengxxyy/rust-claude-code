## Requirements

### Requirement: Markdown headings and lists rendering
The TUI SHALL render common Markdown structural elements with distinct visual treatment, including headings (`#` through `###`) and ordered/unordered lists.

#### Scenario: Render heading with emphasis
- **WHEN** an assistant message contains a level-1 to level-3 Markdown heading
- **THEN** the heading SHALL be displayed with styling that is visually distinct from paragraph text

#### Scenario: Render nested list indentation
- **WHEN** an assistant message contains ordered or unordered list items
- **THEN** the TUI SHALL preserve list structure through indentation and list markers

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

### Requirement: Inline emphasis rendering
The TUI SHALL visually distinguish inline code, bold text, and italic text from surrounding paragraph text.

#### Scenario: Render inline code span
- **WHEN** an assistant message contains inline code delimited by backticks
- **THEN** the inline code span SHALL use a distinct foreground, background, or inverse style

#### Scenario: Render bold and italic text
- **WHEN** an assistant message contains `**bold**` or `*italic*` spans
- **THEN** the TUI SHALL display those spans with visually distinct emphasis styles
