## ADDED Requirements

### Requirement: GrepTool content search
The system SHALL provide a `Grep` tool that searches file contents by regex pattern. The tool SHALL accept a `pattern` parameter (required) and an optional `path` parameter for the search root directory. If `path` is omitted, the tool SHALL use the current working directory.

#### Scenario: Basic content search
- **WHEN** GrepTool is invoked with pattern `fn main`
- **THEN** the tool SHALL return files (or matching lines) containing "fn main" under the search root

#### Scenario: Regex pattern search
- **WHEN** GrepTool is invoked with pattern `fn\s+\w+`
- **THEN** the tool SHALL match using full regex syntax, finding all function definitions

#### Scenario: Search with custom root path
- **WHEN** GrepTool is invoked with pattern `TODO` and path `/some/project/src`
- **THEN** the tool SHALL search only within the specified directory

#### Scenario: No matches found
- **WHEN** GrepTool is invoked with a pattern that matches nothing
- **THEN** the tool SHALL return an empty string result (not an error)

### Requirement: GrepTool output modes
The tool SHALL support an `output_mode` parameter with two modes:
- `files_with_matches` (default): return only the paths of files containing matches
- `content`: return matching lines with line numbers

#### Scenario: Files-only output mode
- **WHEN** GrepTool is invoked with output_mode `files_with_matches`
- **THEN** the tool SHALL return one file path per line for each file containing a match

#### Scenario: Content output mode
- **WHEN** GrepTool is invoked with output_mode `content`
- **THEN** the tool SHALL return matching lines prefixed with file path and line number (format: `path:line:content`)

### Requirement: GrepTool context lines
When output_mode is `content`, the tool SHALL support context line parameters:
- `-A` (integer): lines to show after each match
- `-B` (integer): lines to show before each match
- `-C` (integer): lines to show before and after each match (shorthand for both -A and -B)

#### Scenario: Context lines around match
- **WHEN** GrepTool is invoked with pattern `error`, output_mode `content`, and `-C` set to 2
- **THEN** the tool SHALL show 2 lines before and 2 lines after each matching line

### Requirement: GrepTool file filtering
The tool SHALL support filtering which files are searched:
- `glob` (string, optional): glob pattern to filter files (e.g., `*.rs`, `**/*.ts`)
- `type` (string, optional): file type shorthand (e.g., `rs`, `py`, `js`, `go`, `ts`)

The `type` parameter SHALL map to known file extensions for common programming languages.

#### Scenario: Filter by glob pattern
- **WHEN** GrepTool is invoked with pattern `struct` and glob `*.rs`
- **THEN** the tool SHALL only search in files matching `*.rs`

#### Scenario: Filter by file type
- **WHEN** GrepTool is invoked with pattern `def` and type `py`
- **THEN** the tool SHALL only search in `.py` files

### Requirement: GrepTool result limiting
The tool SHALL support a `head_limit` parameter (integer, optional, default 250) that limits the number of output entries. This prevents enormous result sets from overflowing the context window.

#### Scenario: Default limit applied
- **WHEN** GrepTool finds more than 250 matching entries and no head_limit is specified
- **THEN** the tool SHALL return only the first 250 entries

#### Scenario: Custom limit
- **WHEN** GrepTool is invoked with head_limit set to 10
- **THEN** the tool SHALL return at most 10 entries

### Requirement: GrepTool case sensitivity
The tool SHALL support a case-insensitive flag (`-i`, boolean, optional, default false).

#### Scenario: Case-insensitive search
- **WHEN** GrepTool is invoked with pattern `error` and `-i` set to true
- **THEN** the tool SHALL match `Error`, `ERROR`, `error`, etc.

### Requirement: GrepTool is read-only and concurrency-safe
The `Grep` tool SHALL report `is_read_only() = true` and `is_concurrency_safe() = true`. It MUST NOT modify any files on disk.

#### Scenario: Permission classification
- **WHEN** the permission system checks GrepTool's classification
- **THEN** `is_read_only()` SHALL return `true` and `is_concurrency_safe()` SHALL return `true`

### Requirement: GrepTool tool registration
The `Grep` tool SHALL be registered in the `ToolRegistry` with name `Grep`, a description, and a JSON Schema for its input parameters.

#### Scenario: Tool registered and discoverable
- **WHEN** `ToolRegistry` is initialized with all tools
- **THEN** `registry.get("Grep")` SHALL return the GrepTool with correct info and schema

### Requirement: GrepTool input schema
The tool's `input_schema` SHALL define:
- `pattern` (string, required): the regex pattern to search for
- `path` (string, optional): file or directory to search in
- `glob` (string, optional): glob pattern to filter files
- `type` (string, optional): file type shorthand
- `output_mode` (string, optional, enum: `files_with_matches`, `content`): output format
- `head_limit` (integer, optional): max entries to return
- `-A` (integer, optional): after-context lines
- `-B` (integer, optional): before-context lines
- `-C` (integer, optional): context lines (both directions)
- `-i` (boolean, optional): case-insensitive flag

#### Scenario: Minimal valid input
- **WHEN** GrepTool receives `{"pattern": "TODO"}`
- **THEN** the tool SHALL execute successfully with default settings

#### Scenario: Missing pattern rejected
- **WHEN** GrepTool receives `{}` (no pattern)
- **THEN** the tool SHALL return an InvalidInput error
