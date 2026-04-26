## ADDED Requirements

### Requirement: Discover CLAUDE.local.md alongside CLAUDE.md
The system SHALL discover `CLAUDE.local.md` files in every directory where `CLAUDE.md` discovery is performed, including the global `~/.claude/` directory and all project directories from git root to CWD. Each `CLAUDE.local.md` SHALL be placed immediately after its corresponding `CLAUDE.md` in the ordered result. If `CLAUDE.local.md` exists in a directory but `CLAUDE.md` does not, the local file SHALL still be included at that directory's position.

#### Scenario: CLAUDE.local.md discovered next to CLAUDE.md
- **WHEN** `/repo/CLAUDE.md` and `/repo/CLAUDE.local.md` both exist
- **THEN** the discovery result SHALL include `/repo/CLAUDE.md` followed by `/repo/CLAUDE.local.md`

#### Scenario: CLAUDE.local.md without corresponding CLAUDE.md
- **WHEN** `/repo/sub/CLAUDE.local.md` exists but `/repo/sub/CLAUDE.md` does not
- **THEN** the discovery result SHALL include `/repo/sub/CLAUDE.local.md` at the position corresponding to `/repo/sub/`

#### Scenario: Global CLAUDE.local.md
- **WHEN** `~/.claude/CLAUDE.local.md` exists alongside `~/.claude/CLAUDE.md`
- **THEN** the discovery result SHALL include `~/.claude/CLAUDE.md` followed by `~/.claude/CLAUDE.local.md` before any project files

#### Scenario: Full ordering with local files
- **WHEN** global `~/.claude/CLAUDE.md`, `~/.claude/CLAUDE.local.md`, `/repo/CLAUDE.md`, `/repo/CLAUDE.local.md` all exist, CWD at `/repo`
- **THEN** the returned order SHALL be: `~/.claude/CLAUDE.md`, `~/.claude/CLAUDE.local.md`, `/repo/CLAUDE.md`, `/repo/CLAUDE.local.md`

### Requirement: Discover .claude/rules/*.md path-scoped instruction files
The system SHALL discover markdown files in the project root's `.claude/rules/` directory during CLAUDE.md discovery. Each file MAY contain YAML frontmatter delimited by `---` lines with a `paths` field containing an array of glob patterns. The system SHALL parse the frontmatter to determine path scope.

#### Scenario: Rules directory with multiple files
- **WHEN** `.claude/rules/` contains `frontend.md`, `backend.md`, and `general.md`
- **THEN** the system SHALL discover all three files

#### Scenario: No rules directory
- **WHEN** `.claude/rules/` does not exist
- **THEN** the system SHALL proceed without error and return no rule files

#### Scenario: Parse paths frontmatter
- **WHEN** `.claude/rules/frontend.md` contains frontmatter `---\npaths:\n  - "src/frontend/**"\n  - "src/shared/**"\n---`
- **THEN** the system SHALL parse the paths as `["src/frontend/**", "src/shared/**"]`

#### Scenario: File without frontmatter
- **WHEN** `.claude/rules/general.md` contains no `---` delimited frontmatter
- **THEN** the system SHALL treat the file as having no path scope (always applicable)

## MODIFIED Requirements

### Requirement: Return discovered files in deterministic order
The system SHALL return discovered CLAUDE.md files in the following order: global file first (with its local variant immediately after if present), then project files from root-most to leaf-most (CWD) with each directory's local variant immediately after its public file. Path-scoped rule files from `.claude/rules/` SHALL appear after all CLAUDE.md and CLAUDE.local.md files.

#### Scenario: Full ordering with global and project files
- **WHEN** global `~/.claude/CLAUDE.md` exists and project files exist at `/repo/CLAUDE.md` and `/repo/sub/CLAUDE.md`, with CWD at `/repo/sub`
- **THEN** the returned order SHALL be: `~/.claude/CLAUDE.md`, `/repo/CLAUDE.md`, `/repo/sub/CLAUDE.md`

#### Scenario: Full ordering with local files and rules
- **WHEN** `~/.claude/CLAUDE.md`, `/repo/CLAUDE.md`, `/repo/CLAUDE.local.md`, and `.claude/rules/style.md` all exist, CWD at `/repo`
- **THEN** the returned order SHALL be: `~/.claude/CLAUDE.md`, `/repo/CLAUDE.md`, `/repo/CLAUDE.local.md`, then `.claude/rules/style.md`
