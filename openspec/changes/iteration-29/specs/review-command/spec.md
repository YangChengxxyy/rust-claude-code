## ADDED Requirements

### Requirement: The TUI SHALL provide a `/review` command
The TUI slash command surface SHALL include `/review` as a first-party command for generating structured code review feedback.

#### Scenario: Help lists review command
- **WHEN** the user runs `/help` or views slash command suggestions
- **THEN** `/review` appears with usage that accepts an optional PR number or URL

#### Scenario: Review command is dispatched
- **WHEN** the user enters `/review`
- **THEN** the TUI dispatches a review command instead of sending `/review` as a normal prompt

### Requirement: `/review` SHALL collect changes from the current branch by default
When no argument is supplied, `/review` SHALL inspect the current Git repository and collect a diff for changes on the current branch relative to an appropriate base.

#### Scenario: Current branch has changes
- **WHEN** the user runs `/review` inside a Git repository with changes relative to the base branch
- **THEN** the system collects a diff and submits it for review

#### Scenario: Current branch has no changes
- **WHEN** the user runs `/review` inside a Git repository with no changes to review
- **THEN** the system reports that no reviewable diff was found

#### Scenario: Outside Git repository
- **WHEN** the user runs `/review` outside a Git repository
- **THEN** the system reports that Git repository context is required

### Requirement: `/review` SHALL accept PR number or URL input
When an argument is supplied, `/review` SHALL treat it as a PR number or PR URL and attempt to collect the PR diff using available local tooling.

#### Scenario: PR URL with GitHub CLI available
- **WHEN** the user runs `/review <pr-url>` and `gh` is available
- **THEN** the system collects the PR diff and submits it for review

#### Scenario: PR number with GitHub CLI unavailable
- **WHEN** the user runs `/review <number>` and `gh` is unavailable
- **THEN** the system reports that PR lookup requires `gh` and suggests running `/review` without arguments for local branch review

### Requirement: `/review` SHALL generate structured review output
The review prompt SHALL ask the agent to prioritize correctness bugs, regressions, security issues, and missing tests, with file and line references where available.

#### Scenario: Review finds issues
- **WHEN** the agent identifies review findings from the diff
- **THEN** the response lists findings first, ordered by severity, with concise rationale

#### Scenario: Review finds no issues
- **WHEN** the agent does not identify actionable findings
- **THEN** the response explicitly states that no findings were found and mentions residual risks or testing gaps

### Requirement: `/review` SHALL handle large diffs predictably
The system SHALL bound the amount of diff content sent to the agent and disclose when review input was truncated.

#### Scenario: Diff exceeds review limit
- **WHEN** collected diff content exceeds the configured review input limit
- **THEN** the system truncates the diff deterministically and includes a truncation notice in the review prompt

#### Scenario: Diff is within review limit
- **WHEN** collected diff content is within the configured review input limit
- **THEN** the full collected diff is included in the review prompt
