## MODIFIED Requirements

### Requirement: MCP server configuration model
The system SHALL support an `mcpServers` field in `settings.json`. The field SHALL be a map keyed by server name. Each server definition MUST use a supported `type` value and SHALL preserve the fields for that transport. Stdio server definitions SHALL support `command`, optional `args`, optional `env`, and optional `cwd`. SSE and HTTP server definitions SHALL support `url`, optional `headers`, and optional timeout or reconnect configuration.

#### Scenario: Load minimal stdio server config
- **WHEN** `settings.json` contains `{"mcpServers":{"filesystem":{"type":"stdio","command":"npx"}}}`
- **THEN** the system SHALL load one MCP server named `filesystem` with command `npx`

#### Scenario: Load full stdio server config
- **WHEN** `settings.json` contains a stdio MCP server with `command`, `args`, `env`, and `cwd`
- **THEN** the system SHALL deserialize and preserve all configured fields

#### Scenario: Load SSE server config
- **WHEN** `settings.json` contains `{"mcpServers":{"remote":{"type":"sse","url":"https://example.test/mcp"}}}`
- **THEN** the system SHALL load one MCP server named `remote` using SSE transport and the configured URL

#### Scenario: Load HTTP server config
- **WHEN** `settings.json` contains `{"mcpServers":{"remote":{"type":"http","url":"https://example.test/mcp"}}}`
- **THEN** the system SHALL load one MCP server named `remote` using Streamable HTTP transport and the configured URL

#### Scenario: Unsupported transport type
- **WHEN** `settings.json` contains an MCP server with an unsupported `type`
- **THEN** the system SHALL ignore that server and surface a warning without failing the overall settings load

### Requirement: MCP server config merging across settings layers
When user-level and project-level settings are merged, `mcpServers` SHALL merge by server name.
- Distinct server names SHALL both be preserved
- If the same server name exists in both layers, the higher-priority layer SHALL replace the lower-priority definition for that server

#### Scenario: Merge different server names
- **WHEN** user settings defines `github` and project settings defines `filesystem`
- **THEN** the merged settings SHALL contain both `github` and `filesystem`

#### Scenario: Project overrides user server with same name
- **WHEN** user settings defines `filesystem` with command `a` and project settings defines `filesystem` with command `b`
- **THEN** the merged settings SHALL use the project-level `filesystem` definition with command `b`

### Requirement: MCP runtime metadata exposure
The system SHALL preserve MCP server metadata at runtime, including server name, configured transport type, connection state, and discovered tool list, so that other components can inspect available MCP integrations.

#### Scenario: Connected server metadata is available
- **WHEN** an MCP server initializes successfully and returns two tools
- **THEN** the runtime metadata SHALL record the server as connected and include both discovered tool names

#### Scenario: Failed server metadata is available
- **WHEN** an MCP server fails during startup
- **THEN** the runtime metadata SHALL record the server as failed with an error message

#### Scenario: Remote disconnected metadata is available
- **WHEN** a remote MCP server disconnects after startup
- **THEN** the runtime metadata SHALL record the server as disconnected or reconnecting with an error summary
