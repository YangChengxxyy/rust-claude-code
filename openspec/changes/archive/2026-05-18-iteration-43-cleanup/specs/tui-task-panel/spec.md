## ADDED Requirements

### Requirement: TUI Task side panel
The TUI SHALL provide a Task side panel that displays task entries with ID, status, priority, and a short description while preserving the existing Todo side panel behavior.

#### Scenario: Task panel shows current tasks
- **WHEN** tasks exist in session state or have been delivered through task update events
- **THEN** the Task panel SHALL display each task's ID, status, priority, and description

#### Scenario: Empty Task panel
- **WHEN** no tasks exist
- **THEN** the Task panel SHALL display a clear empty-state placeholder

### Requirement: Task panel can be selected from keyboard
The TUI SHALL support switching side-panel content between Todo and Task views without disabling the existing side-panel visibility behavior.

#### Scenario: Ctrl+T switches panel content
- **WHEN** the user presses `Ctrl+T`
- **THEN** the side panel SHALL switch between Todo and Task content modes

#### Scenario: Tab visibility behavior remains unchanged
- **WHEN** the user presses `Tab`
- **THEN** the side panel visibility SHALL continue to toggle as it did before this change

### Requirement: Task updates refresh the TUI
Task state changes SHALL be reflected by the Task panel on the next render after the TUI receives updated state.

#### Scenario: Task status changes
- **WHEN** a task changes from pending to in-progress or completed
- **THEN** the Task panel SHALL display the new status on the next render
