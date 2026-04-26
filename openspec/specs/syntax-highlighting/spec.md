## Requirements

### Requirement: Syntax highlighting engine initialization
The system SHALL initialize a syntax highlighting engine (syntect) with built-in syntax definitions and a custom theme derived from the current TUI palette, and SHALL lazily initialize this engine as a singleton to avoid repeated construction cost.

#### Scenario: Engine available at first code block
- **WHEN** the first code block in a session needs highlighting
- **THEN** the syntax highlighting engine SHALL be initialized (if not already) and return highlighted output without error

#### Scenario: Singleton reuse
- **WHEN** multiple code blocks are highlighted during a session
- **THEN** the syntax highlighting engine SHALL reuse the same initialized SyntaxSet and Theme instances

### Requirement: Line-level syntax highlighting
The system SHALL highlight source code on a per-line basis, accepting a language identifier and returning styled spans (foreground color + optional modifiers) for each line of code.

#### Scenario: Rust code highlighting
- **WHEN** a code block has language identifier `rust` and contains `fn main() { println!("hello"); }`
- **THEN** `fn` SHALL be highlighted as a keyword, `main` as a function name, and `"hello"` as a string literal, each with distinct colors

#### Scenario: Python code highlighting
- **WHEN** a code block has language identifier `python` and contains `def foo(x): return x + 1`
- **THEN** `def` and `return` SHALL be highlighted as keywords with distinct colors

#### Scenario: JSON highlighting
- **WHEN** a code block has language identifier `json` and contains `{"key": "value", "num": 42}`
- **THEN** string keys, string values, and numeric values SHALL have distinct colors

#### Scenario: Unsupported language fallback
- **WHEN** a code block has an unrecognized language identifier (e.g., `brainfuck`)
- **THEN** the code SHALL be rendered with plain `palette.text` color without errors

#### Scenario: No language identifier
- **WHEN** a code block has no language identifier
- **THEN** the system SHALL attempt plain-text highlighting (no syntax coloring) and render in `palette.text`

### Requirement: Theme derived from TUI palette
The syntax highlighting theme SHALL derive its colors from the current TUI palette so that highlighted code is visually consistent with the rest of the interface in both dark and light modes.

#### Scenario: Dark mode keyword color
- **WHEN** the TUI is in dark mode and a keyword is highlighted
- **THEN** the keyword color SHALL be visually distinguishable from string and comment colors and consistent with the dark palette

#### Scenario: Light mode keyword color
- **WHEN** the TUI is in light mode and a keyword is highlighted
- **THEN** the keyword color SHALL be readable against the light background and visually distinguishable from other token types

#### Scenario: Comment visibility
- **WHEN** a code comment is highlighted
- **THEN** the comment SHALL use a subdued color (derived from `palette.inactive` or similar) that is clearly distinct from active code but still readable

### Requirement: Minimum supported languages
The syntax highlighting engine SHALL support at minimum the following languages: Rust, Python, TypeScript, JavaScript, Go, Java, JSON, YAML, TOML, Markdown, Shell/Bash, C, C++, HTML, CSS.

#### Scenario: All minimum languages recognized
- **WHEN** code blocks with each of the minimum supported language identifiers are encountered
- **THEN** each SHALL be highlighted with language-specific syntax coloring (not plain text)

#### Scenario: Common language aliases
- **WHEN** a code block uses common aliases (`ts` for TypeScript, `py` for Python, `sh` for Shell, `bash` for Bash, `js` for JavaScript, `yml` for YAML)
- **THEN** the alias SHALL resolve to the correct language syntax definition

### Requirement: Highlighting performance
The syntax highlighting engine SHALL highlight a single line in under 1ms on average and a 500-line code block in under 100ms total.

#### Scenario: Large code block performance
- **WHEN** a 500-line Rust source file is highlighted
- **THEN** the total highlighting time SHALL be under 100ms

#### Scenario: Incremental line highlighting
- **WHEN** lines are highlighted one at a time (as in streaming mode)
- **THEN** the per-line highlighting time SHALL average under 1ms, using the previous line's parse state as input
