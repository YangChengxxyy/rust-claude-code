## ADDED Requirements

### Requirement: Discover CLAUDE.md files by walking up from CWD
The system SHALL discover `CLAUDE.md` files by traversing from the current working directory upward through parent directories, stopping at the filesystem root or git repository root. The same git-root boundary and canonical-path normalization logic MUST also be reusable by project-level `.claude/settings.json` discovery so repository-scoped instruction and settings files share a consistent project boundary.

#### Scenario: Single CLAUDE.md in CWD
- **WHEN** the current working directory contains a `CLAUDE.md` file
- **THEN** the system SHALL return a list containing that single file's path and content

#### Scenario: CLAUDE.md in parent directories
- **WHEN** the current working directory is `/a/b/c` and `CLAUDE.md` exists at `/a/CLAUDE.md` and `/a/b/c/CLAUDE.md`
- **THEN** the system SHALL return both files, ordered from root-most (`/a/CLAUDE.md`) to leaf-most (`/a/b/c/CLAUDE.md`)

#### Scenario: No CLAUDE.md found
- **WHEN** no `CLAUDE.md` file exists in the CWD or any parent directory
- **THEN** the system SHALL return an empty list

#### Scenario: Stop at git repository root
- **WHEN** the CWD is inside a git repository and `CLAUDE.md` files exist both inside and outside the repo root
- **THEN** the system SHALL only include `CLAUDE.md` files at or below the git repository root (the directory containing `.git`)

#### Scenario: Shared discovery boundary for project settings
- **WHEN** the system discovers both project-level `.claude/settings.json` and `CLAUDE.md` files from the same working directory inside a git repository
- **THEN** both discovery flows SHALL stop at the same repository root and use the same canonicalized path boundary

### Requirement: Discover global user-level CLAUDE.md
The system SHALL check for a global instruction file at `~/.claude/CLAUDE.md` (or `$CLAUDE_CONFIG_DIR/CLAUDE.md` if the environment variable is set).

#### Scenario: Global CLAUDE.md exists
- **WHEN** `~/.claude/CLAUDE.md` exists and is readable
- **THEN** the system SHALL include it in the discovered files list

#### Scenario: Global CLAUDE.md does not exist
- **WHEN** `~/.claude/CLAUDE.md` does not exist
- **THEN** the system SHALL proceed without error, returning only project-level files

#### Scenario: CLAUDE_CONFIG_DIR override
- **WHEN** the `CLAUDE_CONFIG_DIR` environment variable is set to `/custom/path`
- **THEN** the system SHALL look for the global file at `/custom/path/CLAUDE.md` instead of `~/.claude/CLAUDE.md`

### Requirement: Return discovered files in deterministic order
The system SHALL return discovered CLAUDE.md files in the following order: global file first, then project files from root-most to leaf-most (CWD).

#### Scenario: Full ordering with global and project files
- **WHEN** global `~/.claude/CLAUDE.md` exists and project files exist at `/repo/CLAUDE.md` and `/repo/sub/CLAUDE.md`, with CWD at `/repo/sub`
- **THEN** the returned order SHALL be: `~/.claude/CLAUDE.md`, `/repo/CLAUDE.md`, `/repo/sub/CLAUDE.md`

### Requirement: Handle filesystem errors gracefully
The system SHALL handle unreadable or permission-denied `CLAUDE.md` files gracefully without aborting the entire discovery process.

#### Scenario: Permission denied on one file
- **WHEN** a `CLAUDE.md` file exists but is not readable due to file permissions
- **THEN** the system SHALL skip that file and continue discovering other files

#### Scenario: Symlink resolution
- **WHEN** the directory path contains symbolic links
- **THEN** the system SHALL resolve symlinks to canonical paths to avoid processing the same directory twice
