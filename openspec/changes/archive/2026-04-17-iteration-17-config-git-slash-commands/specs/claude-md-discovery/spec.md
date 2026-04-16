## MODIFIED Requirements

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
