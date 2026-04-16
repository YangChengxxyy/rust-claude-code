## ADDED Requirements

### Requirement: Multi-line input editing
The TUI SHALL support multi-line input editing. Pressing `Shift+Enter` MUST insert a newline at the cursor position, while pressing `Enter` on the input box MUST submit the full buffer as one prompt.

#### Scenario: Insert newline with Shift+Enter
- **WHEN** the user presses `Shift+Enter` while editing input
- **THEN** the TUI SHALL insert a newline at the current cursor position and keep the input buffer in edit mode

#### Scenario: Submit multi-line prompt with Enter
- **WHEN** the input buffer contains multiple lines and the user presses `Enter`
- **THEN** the TUI SHALL submit the entire buffer as a single prompt without flattening embedded newlines

### Requirement: Multi-line paste preservation
The TUI SHALL preserve pasted multi-line content exactly as entered, including newline boundaries and indentation.

#### Scenario: Paste code block into input
- **WHEN** the user pastes text containing multiple lines into the input area
- **THEN** the input buffer SHALL retain the original line breaks and indentation

### Requirement: Input history navigation and persistence
The TUI SHALL record submitted prompts and allow the user to browse prior prompts with `Up` and `Down`. History MUST persist across restarts using a local history file.

#### Scenario: Browse older history entry
- **WHEN** the user presses `Up` in the input area after at least one prior submission
- **THEN** the TUI SHALL replace the current editable buffer with the previous history entry

#### Scenario: Return toward newer history entry
- **WHEN** the user is browsing history and presses `Down`
- **THEN** the TUI SHALL move to the next newer history entry, eventually restoring the in-progress draft buffer

#### Scenario: Reload history after restart
- **WHEN** the user restarts the TUI after previous prompt submissions
- **THEN** submitted prompts stored in the history file SHALL be available for `Up`/`Down` navigation

### Requirement: Advanced cursor movement shortcuts
The TUI SHALL support line and word navigation shortcuts in the input editor, including `Home`, `End`, `Ctrl+A`, `Ctrl+E`, and word-wise movement with `Ctrl+Left` / `Ctrl+Right`.

#### Scenario: Move to line start
- **WHEN** the user presses `Home` or `Ctrl+A`
- **THEN** the cursor SHALL move to the start of the current logical line

#### Scenario: Move by word
- **WHEN** the user presses `Ctrl+Right`
- **THEN** the cursor SHALL advance to the start of the next word boundary within the current line or the next editable segment
