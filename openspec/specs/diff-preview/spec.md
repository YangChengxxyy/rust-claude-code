## Requirements

### Requirement: Diff computation from old and new strings
The system SHALL compute a line-level unified diff from an old string and a new string using a standard diff algorithm (Myers or equivalent), producing a sequence of diff operations (equal, insert, delete) with associated line content.

#### Scenario: Simple single-line replacement
- **WHEN** old_string is `"hello world"` and new_string is `"hello rust"`
- **THEN** the diff SHALL produce a delete operation for `"hello world"` and an insert operation for `"hello rust"`

#### Scenario: Multi-line addition
- **WHEN** old_string is `"line1\nline2"` and new_string is `"line1\ninserted\nline2"`
- **THEN** the diff SHALL produce an equal operation for `"line1"`, an insert operation for `"inserted"`, and an equal operation for `"line2"`

#### Scenario: Empty old_string (new file creation)
- **WHEN** old_string is empty and new_string contains content
- **THEN** the diff SHALL produce insert operations for all lines of new_string

### Requirement: Diff rendering with color coding
The TUI SHALL render computed diffs with visual distinction between added, removed, and unchanged lines, using the palette's diff colors.

#### Scenario: Removed line rendering
- **WHEN** a diff contains a delete operation
- **THEN** the line SHALL be rendered with a `-` prefix and `palette.diff_removed` background color

#### Scenario: Added line rendering
- **WHEN** a diff contains an insert operation
- **THEN** the line SHALL be rendered with a `+` prefix and `palette.diff_added` background color

#### Scenario: Context line rendering
- **WHEN** a diff contains an equal operation
- **THEN** the line SHALL be rendered with a space prefix and default text color

#### Scenario: Line numbers in diff
- **WHEN** a diff is rendered
- **THEN** each line SHALL display its line number (old line number for deletions, new line number for insertions, both for context lines)

### Requirement: Permission dialog shows diff preview for FileEdit
The permission dialog SHALL display a scrollable diff preview when the tool being confirmed is FileEdit, showing the old_string → new_string change with color coding.

#### Scenario: FileEdit permission dialog layout
- **WHEN** a `PermissionRequest` event arrives for the FileEdit tool
- **THEN** the permission dialog SHALL expand to show a header section (tool name, file path, option buttons) and a scrollable diff preview section below

#### Scenario: Diff content extracted from tool input
- **WHEN** a `PermissionRequest` for FileEdit is received with `input` containing `old_string` and `new_string` fields
- **THEN** the diff preview SHALL be computed from these fields without reading the file from disk

#### Scenario: Scroll diff preview
- **WHEN** the diff preview has more lines than the visible area and the user presses Up/Down arrow keys
- **THEN** the diff preview area SHALL scroll to reveal additional diff lines

#### Scenario: Replace_all flag indication
- **WHEN** the FileEdit input has `replace_all: true`
- **THEN** the dialog SHALL display an indicator that all occurrences will be replaced

### Requirement: Permission dialog shows content preview for FileWrite
The permission dialog SHALL display a content preview when the tool being confirmed is FileWrite, showing the first lines of content that will be written.

#### Scenario: FileWrite new file preview
- **WHEN** a `PermissionRequest` event arrives for the FileWrite tool and the target file does not exist
- **THEN** the dialog SHALL show a preview of the content with all lines marked as additions

#### Scenario: FileWrite overwrite indication
- **WHEN** a `PermissionRequest` event arrives for the FileWrite tool and the target file exists
- **THEN** the dialog SHALL indicate that the file will be overwritten and show a preview of the new content

#### Scenario: Content preview truncation
- **WHEN** the FileWrite content exceeds 100 lines
- **THEN** the preview SHALL display the first 100 lines with a truncation indicator showing remaining line count

### Requirement: Diff display in tool result messages
The TUI SHALL display a compact diff summary in the chat area after a FileEdit tool result, so users can review what changed without scrolling back to the permission dialog.

#### Scenario: Tool result with diff summary
- **WHEN** a FileEdit tool completes successfully
- **THEN** the `ToolResult` chat message SHALL display the file path and a compact diff view (showing changed lines with +/- markers and color coding)

#### Scenario: Tool result diff for large changes
- **WHEN** a FileEdit diff has more than 20 changed lines
- **THEN** the tool result SHALL show the first 10 and last 5 changed lines with a collapsed indicator in between

### Requirement: Compact dialog fallback for non-file tools
The permission dialog SHALL retain its original compact layout (tool name, summary, options) for tools that are not FileEdit or FileWrite.

#### Scenario: BashTool permission dialog
- **WHEN** a `PermissionRequest` arrives for the Bash tool
- **THEN** the dialog SHALL use the compact layout without a diff preview section
