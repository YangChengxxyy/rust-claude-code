## ADDED Requirements

### Requirement: Inject CLAUDE.md content into system prompt
The system SHALL inject the content of all discovered CLAUDE.md files into the system prompt as a dedicated `# claudeMd` section.

#### Scenario: Single CLAUDE.md injection
- **WHEN** one CLAUDE.md file is discovered with content "Use conventional commits"
- **THEN** the system prompt SHALL contain a `# claudeMd` section with the text "Use conventional commits"

#### Scenario: Multiple CLAUDE.md files merged
- **WHEN** multiple CLAUDE.md files are discovered (global + project files)
- **THEN** the system prompt SHALL contain a single `# claudeMd` section with all files' content concatenated in discovery order, separated by clear delimiters indicating the source path

#### Scenario: No CLAUDE.md files found
- **WHEN** no CLAUDE.md files are discovered
- **THEN** the system prompt SHALL NOT contain a `# claudeMd` section

### Requirement: Place claudeMd section at correct position in system prompt
The system prompt SHALL place the `# claudeMd` section after the environment information section and before any user-provided custom append content.

#### Scenario: Section ordering in system prompt
- **WHEN** the system prompt is built with tool descriptions, environment info, CLAUDE.md content, and custom append text
- **THEN** the sections SHALL appear in order: core prompt, tool descriptions, environment info, claudeMd, custom append

### Requirement: Truncate CLAUDE.md content when exceeding size limit
The system SHALL enforce a maximum character limit (default 30000 characters) on the total merged CLAUDE.md content. When the limit is exceeded, content SHALL be truncated starting from the global/root-most files to preserve the most specific (leaf-most) project instructions.

#### Scenario: Content within limit
- **WHEN** total merged CLAUDE.md content is 5000 characters (under the 30000 limit)
- **THEN** all content SHALL be included without truncation

#### Scenario: Content exceeds limit
- **WHEN** total merged CLAUDE.md content is 40000 characters, with global file being 15000 characters and project file being 25000 characters
- **THEN** the system SHALL truncate or omit the global file content first, preserving the project-level file content, and include a note indicating truncation occurred

#### Scenario: Single file exceeds limit
- **WHEN** a single CLAUDE.md file is 35000 characters
- **THEN** the system SHALL truncate the file content to fit within the limit and append a truncation indicator

### Requirement: Include source path annotation for each CLAUDE.md
Each CLAUDE.md file's content in the merged output SHALL be annotated with its source path so the model can distinguish between global and project-level instructions.

#### Scenario: Source path annotation format
- **WHEN** CLAUDE.md content from `/repo/CLAUDE.md` is included
- **THEN** the content SHALL be preceded by a header like `Contents of /repo/CLAUDE.md (project instructions, checked into the codebase):` or similar annotation indicating the source
