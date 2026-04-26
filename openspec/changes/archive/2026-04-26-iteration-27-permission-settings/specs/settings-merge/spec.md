## MODIFIED Requirements

### Requirement: Merge project and user settings with deterministic precedence
The system SHALL load settings from project-local `.claude/settings.local.json`, project-shared `.claude/settings.json`, and user-level `~/.claude/settings.json`, then merge them with deterministic precedence: CLI arguments MUST override environment variables, environment variables MUST override project-local settings, project-local settings MUST override project-shared settings, project-shared settings MUST override user settings, and user settings MUST override built-in defaults.

#### Scenario: Project settings override user settings
- **WHEN** the user settings define `model = "claude-sonnet-4-6"` and the project settings define `model = "claude-opus-4-6"`
- **THEN** the effective runtime model SHALL be `claude-opus-4-6`

#### Scenario: Environment overrides project settings
- **WHEN** the project settings define `ANTHROPIC_BASE_URL = "https://project.example"` and the process environment defines `ANTHROPIC_BASE_URL = "https://env.example"`
- **THEN** the effective base URL SHALL be `https://env.example`

#### Scenario: CLI overrides all other sources
- **WHEN** user settings, project settings, and environment variables all define a model, and the CLI is invoked with `--model claude-opus-4-6`
- **THEN** the effective runtime model SHALL be `claude-opus-4-6`

#### Scenario: Local project overrides shared project
- **WHEN** `.claude/settings.json` defines `model = "claude-sonnet-4-6"` and `.claude/settings.local.json` defines `model = "claude-opus-4-6"`
- **THEN** the effective runtime model SHALL be `claude-opus-4-6`

#### Scenario: Permission lists concatenate across all layers
- **WHEN** user settings declare deny rules, shared project declares allow rules, and local project declares additional allow rules
- **THEN** the runtime permission configuration SHALL include all rules from all three layers with their original types preserved

### Requirement: Discover project settings from repository context
The system SHALL discover project-level `.claude/settings.json` and `.claude/settings.local.json` by walking upward from the current working directory and stopping at the git repository root. If no git repository is present, the system SHALL walk to the filesystem root.

#### Scenario: Load project settings from repository root
- **WHEN** the current working directory is `/repo/subdir` and `/repo/.claude/settings.json` exists inside the git repository root
- **THEN** the system SHALL load `/repo/.claude/settings.json` as the project settings file

#### Scenario: Load local project settings alongside shared
- **WHEN** `/repo/.claude/settings.json` and `/repo/.claude/settings.local.json` both exist
- **THEN** the system SHALL load both files, with local settings having higher priority

#### Scenario: Ignore settings above repository root
- **WHEN** the current working directory is inside `/repo`, `/repo/.claude/settings.json` exists, and `/parent/.claude/settings.json` also exists above the git root
- **THEN** the system SHALL only load `/repo/.claude/settings.json` as the project settings file

#### Scenario: No project settings file exists
- **WHEN** no `.claude/settings.json` or `.claude/settings.local.json` exists between the current working directory and the discovery boundary
- **THEN** the system SHALL continue using user settings and defaults without error
