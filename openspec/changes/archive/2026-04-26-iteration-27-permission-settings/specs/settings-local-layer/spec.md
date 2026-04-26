## ADDED Requirements

### Requirement: Discover and load .claude/settings.local.json
The system SHALL discover `.claude/settings.local.json` in the same directory as `.claude/settings.json` during project settings discovery. The local settings file SHALL have higher priority than the shared project settings file.

#### Scenario: Local settings file exists alongside shared settings
- **WHEN** both `.claude/settings.json` and `.claude/settings.local.json` exist in the project's `.claude/` directory
- **THEN** the system SHALL load both files and merge them with local settings taking priority over shared settings

#### Scenario: Local settings file does not exist
- **WHEN** `.claude/settings.json` exists but `.claude/settings.local.json` does not
- **THEN** the system SHALL use the shared project settings without error

#### Scenario: Only local settings file exists
- **WHEN** `.claude/settings.local.json` exists but `.claude/settings.json` does not
- **THEN** the system SHALL load the local file as the project settings layer

### Requirement: Updated settings merge priority chain
The system SHALL merge settings with the following priority (highest to lowest): CLI arguments > environment variables > project-local settings (`.claude/settings.local.json`) > project-shared settings (`.claude/settings.json`) > user settings (`~/.claude/settings.json`) > built-in defaults.

#### Scenario: Local project overrides shared project
- **WHEN** `.claude/settings.json` defines `model = "claude-sonnet-4-6"` and `.claude/settings.local.json` defines `model = "claude-opus-4-6"`
- **THEN** the effective model SHALL be `claude-opus-4-6`

#### Scenario: Environment still overrides local project
- **WHEN** `.claude/settings.local.json` defines `ANTHROPIC_BASE_URL = "https://local.example"` and the process environment defines `ANTHROPIC_BASE_URL = "https://env.example"`
- **THEN** the effective base URL SHALL be `https://env.example`

#### Scenario: Permission lists merge across all layers
- **WHEN** user settings declare deny rule `Bash(rm *)`, shared project declares allow rule `Bash(git *)`, and local project declares allow rule `FileEdit`
- **THEN** the runtime permission configuration SHALL include all three rules with correct types

### Requirement: Discover and inject CLAUDE.local.md
The system SHALL discover `CLAUDE.local.md` files in the same directories where `CLAUDE.md` files are discovered. Each `CLAUDE.local.md` SHALL be inserted immediately after its corresponding `CLAUDE.md` in the ordered file list. `CLAUDE.local.md` at the global level (`~/.claude/CLAUDE.local.md`) SHALL also be discovered.

#### Scenario: CLAUDE.local.md next to CLAUDE.md
- **WHEN** `/repo/CLAUDE.md` and `/repo/CLAUDE.local.md` both exist
- **THEN** the discovery result SHALL include `/repo/CLAUDE.md` followed by `/repo/CLAUDE.local.md` before any deeper directory files

#### Scenario: CLAUDE.local.md without CLAUDE.md
- **WHEN** `/repo/CLAUDE.local.md` exists but `/repo/CLAUDE.md` does not
- **THEN** the system SHALL still include `/repo/CLAUDE.local.md` in the discovery result at its directory position

#### Scenario: Global CLAUDE.local.md
- **WHEN** `~/.claude/CLAUDE.md` and `~/.claude/CLAUDE.local.md` both exist
- **THEN** the discovery result SHALL include `~/.claude/CLAUDE.md` followed by `~/.claude/CLAUDE.local.md` before project files

#### Scenario: CLAUDE.local.md injection annotation
- **WHEN** `CLAUDE.local.md` content is injected into the system prompt
- **THEN** the content SHALL be annotated with a source path header indicating it is a local (user-specific) instruction file

### Requirement: Discover and inject .claude/rules/*.md path-scoped files
The system SHALL discover markdown files in the project root's `.claude/rules/` directory. Each file MAY contain YAML frontmatter with a `paths` field (array of glob patterns). Files with `paths` frontmatter SHALL only be included when the session CWD matches at least one pattern. Files without `paths` frontmatter SHALL always be included.

#### Scenario: Rule file matches CWD
- **WHEN** `.claude/rules/frontend.md` has frontmatter `paths: ["src/frontend/**"]` and the session CWD is `/repo/src/frontend/components`
- **THEN** the system SHALL include the content of `frontend.md` in the system prompt

#### Scenario: Rule file does not match CWD
- **WHEN** `.claude/rules/frontend.md` has frontmatter `paths: ["src/frontend/**"]` and the session CWD is `/repo/src/backend`
- **THEN** the system SHALL NOT include the content of `frontend.md` in the system prompt

#### Scenario: Rule file without paths frontmatter
- **WHEN** `.claude/rules/general.md` has no YAML frontmatter or no `paths` field
- **THEN** the system SHALL always include the content of `general.md` in the system prompt

#### Scenario: Multiple rule files with mixed matching
- **WHEN** `.claude/rules/frontend.md` (paths: `["src/frontend/**"]`) and `.claude/rules/testing.md` (paths: `["tests/**"]`) and `.claude/rules/style.md` (no paths) exist, and the session CWD is `/repo/src/frontend`
- **THEN** the system SHALL include `frontend.md` and `style.md` but NOT `testing.md`

#### Scenario: Rule file content injection position
- **WHEN** path-scoped rule files are included
- **THEN** their content SHALL appear in the `# claudeMd` section after all CLAUDE.md and CLAUDE.local.md files, annotated with their source path
