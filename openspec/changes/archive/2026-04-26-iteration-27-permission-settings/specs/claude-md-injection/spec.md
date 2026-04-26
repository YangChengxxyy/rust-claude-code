## ADDED Requirements

### Requirement: Inject CLAUDE.local.md content with local annotation
The system SHALL inject `CLAUDE.local.md` content into the `# claudeMd` section with a source annotation indicating it is a user-specific local instruction file, distinguishable from the shared `CLAUDE.md` annotation.

#### Scenario: Local file annotation
- **WHEN** `/repo/CLAUDE.local.md` content is injected into the system prompt
- **THEN** the content SHALL be preceded by a header like `Contents of /repo/CLAUDE.local.md (local project instructions, not checked into version control):` or similar annotation

#### Scenario: Local file respects truncation order
- **WHEN** total merged content exceeds the character limit and both `CLAUDE.md` and `CLAUDE.local.md` exist
- **THEN** the truncation priority SHALL still remove global/root-most files first, treating `CLAUDE.local.md` files with the same priority as their corresponding `CLAUDE.md` at the same directory level

### Requirement: Inject path-scoped rule files with CWD matching
The system SHALL evaluate `.claude/rules/*.md` files against the session CWD. Files with `paths` frontmatter SHALL only be injected when the session CWD matches at least one glob pattern (resolved relative to project root). Files without `paths` frontmatter SHALL always be injected. Matching files SHALL appear in the `# claudeMd` section after all CLAUDE.md and CLAUDE.local.md entries.

#### Scenario: CWD matches rule file path pattern
- **WHEN** `.claude/rules/frontend.md` has `paths: ["src/frontend/**"]` and the session CWD is `/repo/src/frontend/components`
- **THEN** the content of `frontend.md` SHALL be injected into the system prompt with a source annotation

#### Scenario: CWD does not match rule file path pattern
- **WHEN** `.claude/rules/frontend.md` has `paths: ["src/frontend/**"]` and the session CWD is `/repo/src/backend`
- **THEN** the content of `frontend.md` SHALL NOT be injected into the system prompt

#### Scenario: Rule file without path restriction
- **WHEN** `.claude/rules/style.md` has no `paths` frontmatter
- **THEN** the content of `style.md` SHALL always be injected into the system prompt

#### Scenario: Rule file annotation
- **WHEN** `.claude/rules/testing.md` content is injected
- **THEN** the content SHALL be preceded by a header like `Contents of .claude/rules/testing.md (project rules):` or similar annotation

#### Scenario: Rule file truncation
- **WHEN** total merged content exceeds the character limit and rule files are included
- **THEN** rule files SHALL be truncated after CLAUDE.local.md files and before leaf-most CLAUDE.md files

## MODIFIED Requirements

### Requirement: Inject CLAUDE.md content into system prompt
The system SHALL inject the content of all discovered CLAUDE.md files, CLAUDE.local.md files, and matched path-scoped rule files into the system prompt as a dedicated `# claudeMd` section.

#### Scenario: Single CLAUDE.md injection
- **WHEN** one CLAUDE.md file is discovered with content "Use conventional commits"
- **THEN** the system prompt SHALL contain a `# claudeMd` section with the text "Use conventional commits"

#### Scenario: Multiple CLAUDE.md files merged
- **WHEN** multiple CLAUDE.md files are discovered (global + project files) along with CLAUDE.local.md and rule files
- **THEN** the system prompt SHALL contain a single `# claudeMd` section with all files' content concatenated in discovery order, separated by clear delimiters indicating the source path and file type

#### Scenario: No CLAUDE.md files found
- **WHEN** no CLAUDE.md, CLAUDE.local.md, or rule files are discovered
- **THEN** the system prompt SHALL NOT contain a `# claudeMd` section

### Requirement: Truncate CLAUDE.md content when exceeding size limit
The system SHALL enforce a maximum character limit (default 30000 characters) on the total merged CLAUDE.md content including CLAUDE.local.md and rule files. When the limit is exceeded, content SHALL be truncated starting from the global/root-most files to preserve the most specific (leaf-most) project instructions. Rule files SHALL be truncated before leaf-most CLAUDE.md files but after root-most files.

#### Scenario: Content within limit
- **WHEN** total merged CLAUDE.md, CLAUDE.local.md, and rule file content is 5000 characters (under the 30000 limit)
- **THEN** all content SHALL be included without truncation

#### Scenario: Content exceeds limit
- **WHEN** total merged content is 40000 characters, with global file being 15000 characters and project file being 25000 characters
- **THEN** the system SHALL truncate or omit the global file content first, preserving the project-level file content, and include a note indicating truncation occurred

#### Scenario: Single file exceeds limit
- **WHEN** a single CLAUDE.md file is 35000 characters
- **THEN** the system SHALL truncate the file content to fit within the limit and append a truncation indicator
