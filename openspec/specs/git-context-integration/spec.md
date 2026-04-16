## ADDED Requirements

### Requirement: Capture Git context for the current workspace
The system SHALL collect a Git context snapshot for the current working directory when the directory is inside a Git repository. The snapshot MUST include the repository root, current branch name, working tree cleanliness, and a bounded summary of recent commits.

#### Scenario: Collect snapshot inside Git repository
- **WHEN** the current working directory is inside a Git repository on branch `main` with a clean working tree
- **THEN** the Git context snapshot SHALL include the repository root, branch `main`, `is_clean = true`, and recent commit summaries

#### Scenario: Dirty working tree is reflected in snapshot
- **WHEN** the current working directory is inside a Git repository with modified or untracked files
- **THEN** the Git context snapshot SHALL set `is_clean = false`

#### Scenario: Outside Git repository
- **WHEN** the current working directory is not inside a Git repository
- **THEN** the system SHALL produce no Git context snapshot and continue without error

### Requirement: Inject Git context into system prompt
The CLI runtime SHALL include the current Git context snapshot in the generated system prompt when Git context is available.

#### Scenario: System prompt includes branch and status
- **WHEN** the runtime builds a system prompt inside a Git repository on branch `feature-x`
- **THEN** the generated system prompt SHALL include the branch name, working tree cleanliness, and recent commit summary information

#### Scenario: No Git context omits Git section
- **WHEN** the runtime builds a system prompt outside a Git repository
- **THEN** the generated system prompt SHALL omit the Git context section

### Requirement: Show Git branch in TUI status bar
The TUI SHALL display the current Git branch in the status bar when a Git context snapshot is available.

#### Scenario: Branch shown in status bar
- **WHEN** the TUI starts inside a Git repository on branch `main`
- **THEN** the status bar SHALL show `main` as part of the current workspace status

#### Scenario: No repository hides branch display
- **WHEN** the TUI starts outside a Git repository
- **THEN** the status bar SHALL omit branch information rather than showing an error placeholder

### Requirement: Refresh Git context for Git-aware commands
Git-aware commands and status rendering MUST use a refreshed or current Git context snapshot so user-visible branch and cleanliness information does not become stale across interactions.

#### Scenario: /diff observes latest working tree state
- **WHEN** the user modifies files after the initial snapshot and then runs `/diff`
- **THEN** the command SHALL use updated Git status and diff information from the current working tree

#### Scenario: Status refresh after branch switch
- **WHEN** the repository branch changes during the session and the UI refreshes Git-aware status
- **THEN** the displayed branch information SHALL update to the current branch
