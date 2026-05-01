# mcp-remote-transports Specification

## Purpose
TBD - created by archiving change iteration-32. Update Purpose after archive.
## Requirements
### Requirement: MCP SSE transport connection
The system SHALL support connecting to configured MCP servers using Server-Sent Events transport. The system SHALL send MCP JSON-RPC requests to the configured remote endpoint and consume JSON-RPC responses from the SSE event stream.

#### Scenario: Successful SSE initialize
- **WHEN** a configured MCP server has `type: "sse"` and a valid remote endpoint
- **THEN** the system SHALL establish an SSE MCP session and send `initialize` before any `tools/list` or `tools/call` request

#### Scenario: SSE connection failure
- **WHEN** the configured SSE endpoint cannot be reached during startup
- **THEN** the system SHALL mark that MCP server as failed and continue starting the rest of the application

### Requirement: MCP Streamable HTTP transport connection
The system SHALL support connecting to configured MCP servers using Streamable HTTP transport. The system SHALL send MCP JSON-RPC requests over HTTP and SHALL parse successful HTTP responses into MCP protocol responses.

#### Scenario: Successful HTTP initialize
- **WHEN** a configured MCP server has `type: "http"` and a valid remote endpoint
- **THEN** the system SHALL establish an HTTP MCP session and send `initialize` before any `tools/list` or `tools/call` request

#### Scenario: HTTP response error
- **WHEN** a Streamable HTTP MCP request returns an HTTP error status or malformed JSON-RPC payload
- **THEN** the system SHALL fail the affected request with a local MCP error and SHALL NOT panic

### Requirement: Remote MCP tool discovery and calls
Remote MCP transports SHALL use the same MCP protocol operations as stdio MCP servers for `tools/list` and `tools/call`. Discovered remote tools SHALL be available for local proxy tool registration using existing MCP tool naming rules.

#### Scenario: Remote server returns tools list
- **WHEN** a connected remote MCP server responds to `tools/list` with tool definitions
- **THEN** the system SHALL persist those tool definitions in MCP runtime metadata and expose them for local tool registration

#### Scenario: Remote tool call succeeds
- **WHEN** the model invokes a local proxy tool backed by a connected remote MCP server
- **THEN** the system SHALL send `tools/call` through that server's configured remote transport and return the MCP response as the local tool result

### Requirement: Remote MCP reconnect handling
The system SHALL attempt to reconnect remote MCP servers after transient transport failures using exponential backoff. Reconnect attempts SHALL update runtime metadata without blocking unrelated MCP servers or built-in tools.

#### Scenario: Remote server reconnects after disconnect
- **WHEN** a remote MCP server disconnects and a later reconnect attempt succeeds
- **THEN** the system SHALL reinitialize that server, refresh its discovered tools, and mark the server as connected

#### Scenario: Tool call during remote disconnect
- **WHEN** the model invokes a proxy tool whose remote MCP server is currently disconnected
- **THEN** the system SHALL return a local tool error indicating that the MCP server is unavailable

### Requirement: Remote MCP timeout handling
The system SHALL enforce configured timeouts for remote MCP initialization, tool discovery, and tool calls. Timed-out remote operations SHALL fail the affected server startup or tool call without crashing the main application.

#### Scenario: Remote initialize timeout
- **WHEN** a remote MCP server does not complete `initialize` within the configured timeout
- **THEN** the system SHALL mark that server as failed or disconnected and continue application startup

#### Scenario: Remote tool call timeout
- **WHEN** a remote MCP `tools/call` request exceeds the configured timeout
- **THEN** the system SHALL return a local tool error indicating timeout

