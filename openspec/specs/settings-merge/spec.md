## ADDED Requirements

### Requirement: Merge project and user settings with deterministic precedence
The system SHALL load settings from project-level `.claude/settings.json` and user-level `~/.claude/settings.json`, then merge them with deterministic precedence: CLI arguments MUST override environment variables, environment variables MUST override project settings, project settings MUST override user settings, and user settings MUST override built-in defaults.

#### Scenario: Project settings override user settings
- **WHEN** the user settings define `model = "claude-sonnet-4-6"` and the project settings define `model = "claude-opus-4-6"`
- **THEN** the effective runtime model SHALL be `claude-opus-4-6`

#### Scenario: Environment overrides project settings
- **WHEN** the project settings define `ANTHROPIC_BASE_URL = "https://project.example"` and the process environment defines `ANTHROPIC_BASE_URL = "https://env.example"`
- **THEN** the effective base URL SHALL be `https://env.example`

#### Scenario: CLI overrides all other sources
- **WHEN** user settings, project settings, and environment variables all define a model, and the CLI is invoked with `--model claude-opus-4-6`
- **THEN** the effective runtime model SHALL be `claude-opus-4-6`

### Requirement: Discover project settings from repository context
The system SHALL discover project-level `.claude/settings.json` by walking upward from the current working directory and stopping at the git repository root. If no git repository is present, the system SHALL walk to the filesystem root.

#### Scenario: Load project settings from repository root
- **WHEN** the current working directory is `/repo/subdir` and `/repo/.claude/settings.json` exists inside the git repository root
- **THEN** the system SHALL load `/repo/.claude/settings.json` as the project settings file

#### Scenario: Ignore settings above repository root
- **WHEN** the current working directory is inside `/repo`, `/repo/.claude/settings.json` exists, and `/parent/.claude/settings.json` also exists above the git root
- **THEN** the system SHALL only load `/repo/.claude/settings.json` as the project settings file

#### Scenario: No project settings file exists
- **WHEN** no `.claude/settings.json` exists between the current working directory and the discovery boundary
- **THEN** the system SHALL continue using user settings and defaults without error

### Requirement: Parse permissions from settings
The system SHALL support declaring permission rules in settings files and merge them into the runtime permission configuration using the same source precedence rules as other settings.

#### Scenario: Project settings add allow rule
- **WHEN** the project settings declare an allow rule for `Bash(git status *)`
- **THEN** the runtime permission configuration SHALL include that allow rule

#### Scenario: Higher-precedence source replaces lower-precedence permission rules
- **WHEN** user settings declare one permission rule set and project settings declare a different permission rule set for the same field
- **THEN** the project settings value SHALL be the effective permission rule set

### Requirement: Preserve configuration source metadata
The system SHALL retain source metadata for effective configuration fields so runtime tooling can explain which source supplied each value.

#### Scenario: /config can explain model source
- **WHEN** the effective model comes from the project settings file
- **THEN** the runtime configuration inspection output SHALL identify the model source as project settings

#### Scenario: Default source is reported
- **WHEN** a configuration field is not set by CLI, environment, project settings, or user settings
- **THEN** the runtime configuration inspection output SHALL identify the field source as built-in default

### Requirement: Validate settings structure before applying
The system SHALL validate supported settings fields before applying them to runtime configuration, and unsupported or malformed values MUST NOT silently change behavior.

#### Scenario: Invalid permission rule structure
- **WHEN** a settings file provides a malformed permission rule entry
- **THEN** the system SHALL report a validation error for that field and MUST NOT apply the malformed rule

#### Scenario: Unknown optional field is ignored safely
- **WHEN** a settings file contains an unknown field outside the supported schema
- **THEN** the system SHALL ignore that field without altering supported configuration behavior
