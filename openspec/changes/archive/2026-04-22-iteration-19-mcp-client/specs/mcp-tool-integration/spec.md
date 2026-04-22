## ADDED Requirements

### Requirement: MCP tools are exposed as local tools
Each discovered MCP tool SHALL be wrapped as a local tool and registered into the existing `ToolRegistry`. The local tool name SHALL use the format `mcp__<server_name>__<tool_name>`.

#### Scenario: Register discovered MCP tool
- **WHEN** server `filesystem` exposes tool `read_file`
- **THEN** the local tool registry SHALL contain a tool named `mcp__filesystem__read_file`

#### Scenario: Register multiple servers with overlapping tool names
- **WHEN** two different MCP servers both expose a tool named `search`
- **THEN** both tools SHALL coexist in the local registry because their fully qualified names include different server names

### Requirement: MCP tool schema and description are forwarded to the model
Each MCP proxy tool SHALL expose the remote tool's description and input schema through the local tool metadata used by `ToolRegistry` and system prompt generation.

#### Scenario: MCP tool appears in system prompt
- **WHEN** a connected MCP server exposes a tool with description and JSON schema
- **THEN** the local system prompt construction SHALL include that MCP tool alongside built-in tools

#### Scenario: MCP tool filtering respects allow/deny lists
- **WHEN** the CLI is started with `--allowed-tools` or `--disallowed-tools`
- **THEN** MCP proxy tools SHALL be filtered by their local tool names using the same logic as built-in tools

### Requirement: MCP tools participate in the existing permission system
MCP proxy tools SHALL go through the same permission checks as built-in tools before execution.

#### Scenario: MCP tool requires confirmation in default mode
- **WHEN** the model invokes an MCP proxy tool in a permission mode that requires confirmation for non-read-only tools
- **THEN** the permission system SHALL evaluate that MCP tool before the call reaches the MCP server

#### Scenario: MCP tool can be matched by permission rules
- **WHEN** a permission rule targets `mcp__filesystem__read_file`
- **THEN** the rule SHALL apply to that MCP proxy tool

### Requirement: `/mcp` slash command displays MCP runtime status
The system SHALL provide a `/mcp` slash command that displays currently configured MCP servers and their runtime status, including connection state and discovered tools.

#### Scenario: Display connected server and tools
- **WHEN** `/mcp` is run and one server is connected with two tools
- **THEN** the output SHALL include the server name, a connected status, and both tool names

#### Scenario: Display failed server
- **WHEN** `/mcp` is run and one configured server failed during startup
- **THEN** the output SHALL include the server name and the recorded failure state or error summary

#### Scenario: No MCP servers configured
- **WHEN** `/mcp` is run and no `mcpServers` are configured
- **THEN** the output SHALL indicate that no MCP servers are configured
