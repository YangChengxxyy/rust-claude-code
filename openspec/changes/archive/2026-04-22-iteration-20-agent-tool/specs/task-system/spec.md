## ADDED Requirements

### Requirement: Task data model
The system SHALL support a `Task` data type with the following fields:
- `id` (string): Unique identifier for the task
- `content` (string): Description of the task
- `status` (enum): One of Pending, InProgress, Completed, Cancelled
- `priority` (enum): One of High, Medium, Low

#### Scenario: Task with all fields
- **WHEN** a task is created with content "Implement login", status Pending, and priority High
- **THEN** the system SHALL store a Task with all fields populated and a generated unique ID

### Requirement: TaskTool with sub-commands
The system SHALL provide a `TaskTool` that accepts a `command` field to distinguish operations. Supported commands SHALL be: `create`, `list`, `update`, `get`.

#### Scenario: Create task
- **WHEN** TaskTool is invoked with `command: "create"` and `content: "Fix bug #42"`, `priority: "high"`
- **THEN** the system SHALL create a new task with status Pending and return the created task's ID and details

#### Scenario: List tasks
- **WHEN** TaskTool is invoked with `command: "list"`
- **THEN** the system SHALL return all tasks with their ID, content, status, and priority

#### Scenario: Update task status
- **WHEN** TaskTool is invoked with `command: "update"`, `id: "task_1"`, and `status: "completed"`
- **THEN** the system SHALL update the specified task's status and return the updated task

#### Scenario: Get task details
- **WHEN** TaskTool is invoked with `command: "get"` and `id: "task_1"`
- **THEN** the system SHALL return the full details of the specified task

#### Scenario: Update non-existent task
- **WHEN** TaskTool is invoked with `command: "update"` and a non-existent task ID
- **THEN** the system SHALL return an error indicating the task was not found

#### Scenario: Invalid command
- **WHEN** TaskTool is invoked with an unrecognized command
- **THEN** the system SHALL return an error listing the valid commands

### Requirement: Task storage in AppState
Tasks SHALL be stored in `AppState` as an in-memory collection, accessible via `Arc<Mutex<AppState>>`. The storage SHALL support concurrent access from multiple tools.

#### Scenario: Tasks persist across tool calls
- **WHEN** a task is created in one tool call and listed in a subsequent tool call
- **THEN** the created task SHALL appear in the list

#### Scenario: Tasks survive across QueryLoop rounds
- **WHEN** a task is created in round 1 and queried in round 3
- **THEN** the task SHALL still be present with its last-known status

### Requirement: TaskTool replaces TodoWriteTool
The system SHALL register `TaskTool` in place of `TodoWriteTool`. The tool name exposed to the model SHALL be `Task`. The system SHALL NOT register both TodoWriteTool and TaskTool simultaneously.

#### Scenario: Only TaskTool is registered
- **WHEN** the ToolRegistry is initialized
- **THEN** it SHALL contain `Task` but NOT `TodoWrite`

### Requirement: TaskTool is non-read-only for create/update operations
TaskTool's `create` and `update` commands SHALL be treated as non-read-only since they modify state. The `list` and `get` commands are read-only by nature but the tool as a whole SHALL be registered as non-read-only.

#### Scenario: TaskTool permission in plan mode
- **WHEN** permission mode is Plan and the model invokes TaskTool with command "create"
- **THEN** the system SHALL deny the operation (Plan mode blocks non-read-only tools)

### Requirement: TUI Task panel displays tasks
The TUI SHALL display tasks in the side panel (replacing the todo panel). Each task SHALL show its status icon, priority indicator, and content text.

#### Scenario: Task panel shows status icons
- **WHEN** tasks exist with various statuses (Pending, InProgress, Completed)
- **THEN** the panel SHALL display distinct icons for each status (e.g., circle variants)

#### Scenario: Empty task panel
- **WHEN** no tasks exist
- **THEN** the panel SHALL display a "No tasks" placeholder message

### Requirement: Task updates propagate to TUI
When tasks are created or updated via TaskTool, the system SHALL emit an event to update the TUI's task panel display.

#### Scenario: Real-time task update
- **WHEN** a task status changes from Pending to InProgress during a tool execution
- **THEN** the TUI task panel SHALL reflect the updated status on next render
