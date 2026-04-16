## MODIFIED Requirements

### Requirement: Input history navigation and persistence
The TUI SHALL record submitted prompts and allow the user to browse prior prompts with `Up` and `Down`. History MUST persist across restarts using a local history file. Chat viewport scrolling shortcuts MUST NOT overwrite the current editable draft or interfere with `Up` / `Down` history navigation semantics.

#### Scenario: Browse older history entry
- **WHEN** the user presses `Up` in the input area after at least one prior submission
- **THEN** the TUI SHALL replace the current editable buffer with the previous history entry

#### Scenario: Return toward newer history entry
- **WHEN** the user is browsing history and presses `Down`
- **THEN** the TUI SHALL move to the next newer history entry, eventually restoring the in-progress draft buffer

#### Scenario: Reload history after restart
- **WHEN** the user restarts the TUI after previous prompt submissions
- **THEN** submitted prompts stored in the history file SHALL be available for `Up` / `Down` navigation

#### Scenario: Chat scrolling preserves current draft
- **WHEN** the user scrolls the chat viewport with dedicated chat navigation shortcuts while an input draft is present
- **THEN** the TUI SHALL preserve the current input buffer contents, cursor position, and history browsing state
