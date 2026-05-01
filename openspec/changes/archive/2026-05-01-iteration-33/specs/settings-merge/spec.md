## MODIFIED Requirements

### Requirement: Merge project and user settings with deterministic precedence
The system SHALL load settings from project-level `.claude/settings.json` and user-level `~/.claude/settings.json`, then merge them with deterministic precedence: CLI arguments MUST override environment variables, environment variables MUST override project settings, project settings MUST override user settings, and user settings MUST override built-in defaults. Sandbox configuration fields MUST follow the same precedence rules as other runtime settings.

#### Scenario: Project settings override user settings
- **WHEN** the user settings define `model = "claude-sonnet-4-6"` and the project settings define `model = "claude-opus-4-6"`
- **THEN** the effective runtime model SHALL be `claude-opus-4-6`

#### Scenario: Environment overrides project settings
- **WHEN** the project settings define `ANTHROPIC_BASE_URL = "https://project.example"` and the process environment defines `ANTHROPIC_BASE_URL = "https://env.example"`
- **THEN** the effective base URL SHALL be `https://env.example`

#### Scenario: CLI overrides all other sources
- **WHEN** user settings, project settings, and environment variables all define a model, and the CLI is invoked with `--model claude-opus-4-6`
- **THEN** the effective runtime model SHALL be `claude-opus-4-6`

#### Scenario: CLI sandbox flag overrides settings
- **WHEN** user settings disable sandboxing, project settings enable sandboxing, and the CLI explicitly disables sandboxing
- **THEN** the effective sandbox enabled value SHALL be disabled

#### Scenario: Environment sandbox paths override project settings
- **WHEN** project settings define sandbox allowed paths and the process environment defines sandbox allowed paths
- **THEN** the effective sandbox allowed paths SHALL come from the environment value

### Requirement: Validate settings structure before applying
The system SHALL validate supported settings fields before applying them to runtime configuration, and unsupported or malformed values MUST NOT silently change behavior. Sandbox settings MUST validate boolean fields, network policy values, and allowed path structures before they are applied.

#### Scenario: Invalid permission rule structure
- **WHEN** a settings file provides a malformed permission rule entry
- **THEN** the system SHALL report a validation error for that field and MUST NOT apply the malformed rule

#### Scenario: Unknown optional field is ignored safely
- **WHEN** a settings file contains an unknown field outside the supported schema
- **THEN** the system SHALL ignore that field without altering supported configuration behavior

#### Scenario: Invalid sandbox network policy is rejected
- **WHEN** a settings file provides an unsupported sandbox network policy value
- **THEN** the system SHALL report a validation error for that field and MUST NOT apply the malformed sandbox configuration

#### Scenario: Invalid sandbox allowed path is rejected
- **WHEN** a settings file provides a sandbox allowed path entry that cannot be parsed as a path string
- **THEN** the system SHALL report a validation error for that field and MUST NOT apply the malformed sandbox configuration
