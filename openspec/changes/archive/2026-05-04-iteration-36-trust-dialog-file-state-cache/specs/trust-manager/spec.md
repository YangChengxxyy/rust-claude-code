## ADDED Requirements

### Requirement: Trust status check
The system SHALL check whether the current working directory is trusted before executing `apiKeyHelper` or loading `settings.json` `env` variables from project-level settings. `TrustManager::check_trust(project_dir)` SHALL return one of: `Trusted`, `Untrusted`, or `InheritedFromParent`.

#### Scenario: Directory is explicitly trusted
- **WHEN** the user has previously accepted trust for `/Users/cc/projects/my-project`
- **THEN** `check_trust("/Users/cc/projects/my-project")` SHALL return `Trusted`

#### Scenario: Directory inherits trust from parent
- **WHEN** `/Users/cc/projects` is trusted and the current directory is `/Users/cc/projects/sub-project`
- **THEN** `check_trust("/Users/cc/projects/sub-project")` SHALL return `InheritedFromParent`

#### Scenario: Directory is not trusted
- **WHEN** no trust record exists for `/Users/cc/untrusted` or any of its parent directories
- **THEN** `check_trust("/Users/cc/untrusted")` SHALL return `Untrusted`

### Requirement: Trust acceptance and persistence
The system SHALL persist trust decisions to `~/.config/rust-claude-code/trust.json`. When a user accepts trust for a directory, `TrustManager::accept_trust(project_dir)` SHALL write the canonical path and timestamp to the trust file.

#### Scenario: Accept trust for a regular directory
- **WHEN** the user confirms trust for `/Users/cc/projects/new-project`
- **THEN** the system SHALL write an entry to `trust.json` with the canonical path and current timestamp
- **AND** subsequent calls to `check_trust("/Users/cc/projects/new-project")` SHALL return `Trusted`

#### Scenario: Accept trust for home directory
- **WHEN** the user confirms trust for their home directory (`~`)
- **THEN** the system SHALL NOT persist the trust decision to `trust.json`
- **AND** trust SHALL remain valid only for the current process lifetime

### Requirement: Trust gate for risky settings
The system SHALL only gate trust when project-level `.claude/settings.json` contains `apiKeyHelper` or `env` fields. If neither field is present, the trust check SHALL be skipped.

#### Scenario: Project has apiKeyHelper — untrusted
- **WHEN** the project `.claude/settings.json` contains `apiKeyHelper: "some-command"`
- **AND** the directory is untrusted
- **THEN** the system SHALL NOT execute `apiKeyHelper`
- **AND** SHALL NOT load `env` variables from project settings

#### Scenario: Project has no risky fields
- **WHEN** the project `.claude/settings.json` contains only safe fields (e.g., `model`, `permissions`)
- **THEN** the system SHALL skip the trust check entirely and proceed normally

#### Scenario: Project has env variables — untrusted
- **WHEN** the project `.claude/settings.json` contains `env: { "FOO": "bar" }`
- **AND** the directory is untrusted
- **THEN** the system SHALL NOT apply the `env` variables from project settings

### Requirement: TUI trust confirmation dialog
When running in TUI mode in an untrusted directory with risky settings, the system SHALL display a trust confirmation dialog before proceeding. The dialog SHALL show the project path and a security warning.

#### Scenario: User confirms trust in TUI
- **WHEN** the trust dialog is shown for `/Users/cc/projects/new-project`
- **AND** the user selects "Trust" (or presses `y`)
- **THEN** the system SHALL call `accept_trust("/Users/cc/projects/new-project")`
- **AND** SHALL proceed with loading `apiKeyHelper` and `env`

#### Scenario: User denies trust in TUI
- **WHEN** the trust dialog is shown
- **AND** the user selects "Don't Trust" (or presses `n` / `Esc`)
- **THEN** the system SHALL NOT execute `apiKeyHelper` or load project `env`
- **AND** SHALL continue startup with user-level settings only

### Requirement: Non-interactive trust handling
In `--print` mode (non-interactive), the system SHALL refuse to run in untrusted directories with risky settings unless `--trust` is provided.

#### Scenario: Print mode in untrusted directory without --trust
- **WHEN** running with `--print` in an untrusted directory with `apiKeyHelper`
- **AND** `--trust` is NOT provided
- **THEN** the system SHALL print an error message suggesting `--trust`
- **AND** SHALL exit with a non-zero status code

#### Scenario: Print mode with --trust flag
- **WHEN** running with `--print --trust` in an untrusted directory
- **THEN** the system SHALL proceed as if the directory is trusted
- **AND** SHALL NOT persist the trust decision

### Requirement: --trust CLI flag
The system SHALL accept a `--trust` CLI flag that bypasses the trust dialog for the current invocation without persisting the trust decision.

#### Scenario: --trust flag skips dialog
- **WHEN** running with `--trust` in any untrusted directory
- **THEN** the system SHALL skip the trust dialog
- **AND** SHALL proceed with loading all settings including `apiKeyHelper` and `env`
- **AND** SHALL NOT write to `trust.json`
