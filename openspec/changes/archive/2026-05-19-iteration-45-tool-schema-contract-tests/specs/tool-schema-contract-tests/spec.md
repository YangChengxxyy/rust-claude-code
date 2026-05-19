## ADDED Requirements

### Requirement: Built-in tool schema contracts
The system SHALL provide tests that summarize built-in tool schemas in a stable order. The summary SHALL include each tool name, whether it is deferred, required fields, and top-level property names.

#### Scenario: Schema summary is stable
- **WHEN** the schema contract test builds a registry containing core tools
- **THEN** the produced summary SHALL match the expected inline contract

#### Scenario: Schema-breaking change fails test
- **WHEN** a core tool changes its required fields or top-level input properties
- **THEN** the schema contract test SHALL fail until the expected contract is intentionally updated

### Requirement: File tool path aliases
The system SHALL accept both `path` and `file_path` for `FileRead`, `FileEdit`, and `FileWrite` inputs. Internal execution SHALL continue to use a single canonical path value.

#### Scenario: FileRead accepts path
- **WHEN** `FileRead` is invoked with `path`
- **THEN** the tool SHALL read the requested file

#### Scenario: FileRead accepts file_path
- **WHEN** `FileRead` is invoked with `file_path`
- **THEN** the tool SHALL read the requested file

#### Scenario: FileEdit accepts file_path
- **WHEN** `FileEdit` is invoked with `file_path`, `old_string`, and `new_string`
- **THEN** the tool SHALL edit the requested file

#### Scenario: FileWrite accepts file_path
- **WHEN** `FileWrite` is invoked with `file_path` and `content`
- **THEN** the tool SHALL write the requested file

#### Scenario: File tool requires a path alias
- **WHEN** a file tool invocation omits both `path` and `file_path`
- **THEN** deserialization SHALL reject the input as invalid

### Requirement: Deferred tool schema contract
The system SHALL test that deferred tools expose full schema definitions through the deferred discovery path while non-deferred tools remain excluded from deferred search results.

#### Scenario: Deferred tool search returns full schema
- **WHEN** `ToolSearch` finds a deferred tool
- **THEN** the returned JSON SHALL include the tool `name`, `description`, and `input_schema`

#### Scenario: Non-deferred tool is excluded from deferred search
- **WHEN** `ToolSearch` is invoked with `select:Bash`
- **THEN** it SHALL return no schema because `Bash` is not deferred
