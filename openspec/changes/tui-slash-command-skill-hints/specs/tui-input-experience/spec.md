## MODIFIED Requirements

### Requirement: Input history navigation and persistence
The TUI SHALL record submitted prompts and allow the user to browse prior prompts with `Up` and `Down`. History MUST persist across restarts using a local history file. Chat viewport scrolling shortcuts MUST NOT overwrite the current editable draft or interfere with `Up` / `Down` history navigation semantics. When a slash suggestion overlay is visible and has at least one selectable candidate, `Up` and `Down` MUST navigate the suggestion list instead of browsing history.

#### Scenario: Browse older history entry
- **WHEN** the user presses `Up` in the input area after at least one prior submission and no active slash suggestion selection is visible
- **THEN** the TUI SHALL replace the current editable buffer with the previous history entry

#### Scenario: Return toward newer history entry
- **WHEN** the user is browsing history and presses `Down` while no active slash suggestion selection is visible
- **THEN** the TUI SHALL move to the next newer history entry, eventually restoring the in-progress draft buffer

#### Scenario: Reload history after restart
- **WHEN** the user restarts the TUI after previous prompt submissions
- **THEN** submitted prompts stored in the history file SHALL be available for `Up` / `Down` navigation

#### Scenario: Chat scrolling preserves current draft
- **WHEN** the user scrolls the chat viewport with dedicated chat navigation shortcuts while an input draft is present
- **THEN** the TUI SHALL preserve the current input buffer contents, cursor position, and history browsing state

#### Scenario: Suggestion overlay captures arrow navigation
- **WHEN** the input buffer begins with `/` and the suggestion overlay contains selectable candidates
- **THEN** pressing `Up` or `Down` SHALL move the suggestion selection instead of changing input history state

## ADDED Requirements

### Requirement: Slash suggestion overlay appears above the input area
The TUI SHALL display a suggestion overlay above the input area whenever the current input buffer begins with `/`. The overlay SHALL show grouped command and skill suggestions aligned in columns and filtered by the current slash query prefix.

#### Scenario: Show all suggestions for bare slash
- **WHEN** the user types `/` into an otherwise empty input buffer
- **THEN** the TUI SHALL display a suggestion overlay containing both command and skill groups

#### Scenario: Filter suggestions by prefix
- **WHEN** the user types a slash query prefix such as `/he`
- **THEN** the overlay SHALL reduce the visible candidates to command and skill entries whose configured match fields contain the prefix

#### Scenario: Hide overlay for non-slash input
- **WHEN** the input buffer no longer begins with `/`
- **THEN** the suggestion overlay SHALL be hidden and normal input behavior SHALL resume

### Requirement: Slash suggestion selection applies the highlighted candidate
The TUI SHALL allow the user to choose a visible suggestion with keyboard navigation and apply it to the input buffer without immediately submitting the message.

#### Scenario: Apply selected command candidate
- **WHEN** a slash command suggestion is highlighted and the user presses `Enter`
- **THEN** the TUI SHALL replace the current slash query with the selected command token and SHALL NOT submit the input in the same keypress

#### Scenario: Apply selected skill candidate
- **WHEN** a skill suggestion is highlighted and the user presses `Enter`
- **THEN** the TUI SHALL replace the current slash query with the configured skill insertion text and SHALL NOT submit the input in the same keypress

#### Scenario: Cancel suggestion overlay
- **WHEN** the suggestion overlay is visible and the user presses `Esc`
- **THEN** the TUI SHALL close the overlay without clearing the current input buffer contents
