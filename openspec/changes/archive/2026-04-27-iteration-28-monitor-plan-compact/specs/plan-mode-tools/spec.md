## ADDED Requirements

### Requirement: Enter plan mode by tool call
The system SHALL provide an `EnterPlanModeTool` that switches the shared application permission mode to `Plan` and records the previous permission mode for later restoration.

#### Scenario: Agent enters plan mode
- **WHEN** the agent invokes `EnterPlanModeTool` while the current permission mode is not `Plan`
- **THEN** the system SHALL save the current permission mode, set the active mode to `Plan`, and return a tool result confirming the transition

#### Scenario: Enter plan mode while already planning
- **WHEN** the agent invokes `EnterPlanModeTool` while the active permission mode is already `Plan`
- **THEN** the system SHALL keep the active mode as `Plan` and return a tool result indicating no additional transition was needed

### Requirement: Exit plan mode by tool call
The system SHALL provide an `ExitPlanModeTool` that accepts a plan summary, restores the previously saved permission mode, and clears the saved plan-mode transition state.

#### Scenario: Agent exits plan mode
- **WHEN** the agent invokes `ExitPlanModeTool` with a non-empty plan summary after entering plan mode
- **THEN** the system SHALL restore the saved permission mode and return a tool result containing the submitted plan summary

#### Scenario: Exit without saved mode
- **WHEN** the agent invokes `ExitPlanModeTool` and no previous permission mode is saved
- **THEN** the system SHALL leave the current permission mode unchanged and return a tool result explaining that no plan-mode transition was active

### Requirement: Plan mode remains read-only
While plan mode is active, the system SHALL continue to block non-read-only tools according to existing plan-mode permission semantics, including tools invoked after `EnterPlanModeTool` succeeds.

#### Scenario: Mutating tool blocked in plan mode
- **WHEN** the agent enters plan mode and then invokes a non-read-only tool
- **THEN** the permission system SHALL deny the non-read-only tool call
