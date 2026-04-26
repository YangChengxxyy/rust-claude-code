## ADDED Requirements

### Requirement: AskUserQuestionTool asks structured user questions
The system SHALL provide an `AskUserQuestion` tool that lets the model ask the user a structured question during the tool loop.

#### Scenario: Tool schema exposes question and options
- **WHEN** tools are listed for the model
- **THEN** `AskUserQuestion` SHALL expose a schema with `question`, `options`, and `allow_custom`

#### Scenario: User selects an option
- **WHEN** the model invokes `AskUserQuestion` with a question and labeled options
- **AND** the user selects one option
- **THEN** the tool result SHALL include the selected option label and answer text

#### Scenario: User provides custom answer
- **WHEN** the model invokes `AskUserQuestion` with `allow_custom: true`
- **AND** the user enters a custom answer
- **THEN** the tool result SHALL include the custom answer text

### Requirement: AskUserQuestionTool integrates with the TUI
The TUI SHALL display AskUserQuestion requests as interactive modal prompts and return the user's response through the tool execution path.

#### Scenario: TUI modal displays choices
- **WHEN** an AskUserQuestion request reaches the TUI bridge
- **THEN** the TUI SHALL render the question and available options in a modal
- **AND** keyboard navigation SHALL allow selecting an option and submitting it

#### Scenario: User cancels question
- **WHEN** the user cancels the AskUserQuestion modal
- **THEN** the tool result SHALL indicate that the question was cancelled or unavailable

#### Scenario: Active stream waits for answer
- **WHEN** AskUserQuestion is invoked during an agent turn
- **THEN** the QueryLoop SHALL wait for the user response before appending the tool result and continuing

### Requirement: AskUserQuestionTool has deterministic non-interactive behavior
The system SHALL avoid hanging when AskUserQuestion runs without an interactive TUI response path.

#### Scenario: Non-interactive fallback with options
- **WHEN** AskUserQuestion executes without an interactive response path
- **AND** at least one option is provided
- **THEN** the tool SHALL return the first option as the deterministic fallback answer

#### Scenario: Non-interactive fallback without answer source
- **WHEN** AskUserQuestion executes without an interactive response path
- **AND** no option or custom answer source is available
- **THEN** the tool SHALL return an error result instead of waiting indefinitely

