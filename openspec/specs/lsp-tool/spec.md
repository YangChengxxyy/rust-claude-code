## ADDED Requirements

### Requirement: LspTool supports semantic code navigation
The system SHALL provide an `LspTool` that can perform semantic code navigation using language servers. Supported operations SHALL include `goToDefinition`, `findReferences`, `hover`, `documentSymbol`, and `workspaceSymbol`.

#### Scenario: Go to definition in Rust project
- **WHEN** the model invokes `LspTool` with `operation: "goToDefinition"` on a Rust symbol
- **THEN** the system SHALL return the file path, line, and column of the symbol definition

#### Scenario: Find references
- **WHEN** the model invokes `LspTool` with `operation: "findReferences"` on a symbol
- **THEN** the system SHALL return all matching reference locations in a readable list

#### Scenario: Hover information
- **WHEN** the model invokes `LspTool` with `operation: "hover"` on a symbol
- **THEN** the system SHALL return the hover text or type information provided by the language server

### Requirement: LspTool starts language servers on demand
The system SHALL start the appropriate language server on demand based on the target file or workspace language. The first supported languages SHALL be Rust, TypeScript, and Python.

#### Scenario: Rust file triggers rust-analyzer
- **WHEN** `LspTool` is used on a `.rs` file
- **THEN** the system SHALL start or reuse `rust-analyzer`

#### Scenario: Unsupported language
- **WHEN** `LspTool` is used on a file type with no configured language server
- **THEN** the system SHALL return an error indicating that no language server is available

### Requirement: LspTool communicates over JSON-RPC stdio
The system SHALL communicate with language servers over JSON-RPC using stdio transport.

#### Scenario: Successful initialize and request
- **WHEN** a language server starts successfully
- **THEN** the system SHALL initialize the LSP session before sending navigation requests

#### Scenario: Language server start failure
- **WHEN** the configured language server executable cannot be started
- **THEN** the system SHALL return a tool error result without crashing the main application
