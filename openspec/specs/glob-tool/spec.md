## ADDED Requirements

### Requirement: GlobTool file pattern matching
The system SHALL provide a `Glob` tool that searches for files matching a glob pattern within a specified directory. The tool SHALL accept a `pattern` parameter (required) and an optional `path` parameter for the search root directory. If `path` is omitted, the tool SHALL use the current working directory.

#### Scenario: Basic glob search
- **WHEN** GlobTool is invoked with pattern `**/*.rs`
- **THEN** the tool SHALL return all `.rs` files found recursively under the search root, one file path per line

#### Scenario: Search with custom root path
- **WHEN** GlobTool is invoked with pattern `*.toml` and path `/some/project`
- **THEN** the tool SHALL search only within `/some/project` and return matching `.toml` files

#### Scenario: No matches found
- **WHEN** GlobTool is invoked with a pattern that matches no files
- **THEN** the tool SHALL return an empty string result (not an error)

### Requirement: GlobTool result sorting
The tool SHALL return matched file paths sorted by modification time (most recently modified first). This surfaces the most relevant files at the top of the result list.

#### Scenario: Results sorted by mtime
- **WHEN** GlobTool matches multiple files with different modification times
- **THEN** the results SHALL be ordered with the most recently modified file first

### Requirement: GlobTool is read-only and concurrency-safe
The `Glob` tool SHALL report `is_read_only() = true` and `is_concurrency_safe() = true`. It MUST NOT modify any files on disk.

#### Scenario: Permission classification
- **WHEN** the permission system checks GlobTool's classification
- **THEN** `is_read_only()` SHALL return `true` and `is_concurrency_safe()` SHALL return `true`

### Requirement: GlobTool tool registration
The `Glob` tool SHALL be registered in the `ToolRegistry` with name `Glob`, a description of its purpose, and a JSON Schema for its input parameters (`pattern`: string required, `path`: string optional).

#### Scenario: Tool registered and discoverable
- **WHEN** `ToolRegistry` is initialized with all tools
- **THEN** `registry.get("Glob")` SHALL return the GlobTool with correct info and schema

### Requirement: GlobTool input schema
The tool's `input_schema` SHALL define:
- `pattern` (string, required): the glob pattern to match files against
- `path` (string, optional): the directory to search in, defaults to current working directory

#### Scenario: Valid input accepted
- **WHEN** GlobTool receives `{"pattern": "**/*.rs"}`
- **THEN** the tool SHALL execute successfully

#### Scenario: Missing pattern rejected
- **WHEN** GlobTool receives `{}` (no pattern)
- **THEN** the tool SHALL return an InvalidInput error
