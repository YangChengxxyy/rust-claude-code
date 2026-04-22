## ADDED Requirements

### Requirement: MCP stdio server startup and initialization
The system SHALL start each configured stdio MCP server as a child process and establish a JSON-RPC session over the server's stdin/stdout streams. After process start, the system SHALL send an `initialize` request before any other MCP request.

#### Scenario: Successful stdio startup and initialize
- **WHEN** a configured stdio MCP server process starts successfully and returns a valid `initialize` response
- **THEN** the system SHALL mark the server as connected and ready for subsequent MCP requests

#### Scenario: Process start failure
- **WHEN** the configured MCP server executable cannot be started
- **THEN** the system SHALL mark that server as failed and continue starting the rest of the application

#### Scenario: Initialize failure
- **WHEN** the MCP server process starts but returns an invalid or error `initialize` response
- **THEN** the system SHALL mark that server as failed and SHALL NOT issue `tools/list` or `tools/call` to that server

### Requirement: MCP JSON-RPC request handling for tools/list
For each successfully initialized MCP server, the system SHALL send a `tools/list` request and parse the server's tool definitions.

#### Scenario: Server returns tools list
- **WHEN** a connected MCP server responds to `tools/list` with multiple tool definitions
- **THEN** the system SHALL persist those tool definitions in the MCP runtime state

#### Scenario: Server returns empty tools list
- **WHEN** a connected MCP server responds to `tools/list` with no tools
- **THEN** the system SHALL keep the server connected and expose zero MCP tools for that server

#### Scenario: tools/list request fails
- **WHEN** a connected MCP server returns an error or malformed response for `tools/list`
- **THEN** the system SHALL mark the server as failed for tool exposure and surface an error for observability

### Requirement: MCP JSON-RPC request handling for tools/call
The system SHALL support invoking MCP tools by sending a `tools/call` request to the corresponding connected server. The request SHALL include the MCP tool name and JSON arguments provided by the model.

#### Scenario: Successful MCP tool call
- **WHEN** the model invokes an MCP proxy tool with valid JSON input and the server returns a successful `tools/call` response
- **THEN** the system SHALL convert that response into a successful local tool result

#### Scenario: MCP tool call returns server error
- **WHEN** the server returns an MCP error response for `tools/call`
- **THEN** the system SHALL convert it into a local tool error result with the server error message

#### Scenario: MCP tool call on disconnected server
- **WHEN** the model invokes an MCP proxy tool whose backing server is not connected
- **THEN** the system SHALL return a local tool error result indicating the MCP server is unavailable

### Requirement: Timeout and protocol error handling
The system SHALL enforce timeouts on MCP initialization and tool calls. Invalid JSON-RPC frames, malformed JSON payloads, or timed-out operations SHALL fail the affected server or tool call without crashing the main application.

#### Scenario: Initialize times out
- **WHEN** an MCP server does not complete `initialize` within the configured timeout
- **THEN** the system SHALL terminate or detach that startup attempt, mark the server as failed, and continue application startup

#### Scenario: Tool call times out
- **WHEN** an MCP `tools/call` request exceeds the configured timeout
- **THEN** the system SHALL return a local tool error result indicating timeout

#### Scenario: Invalid JSON-RPC frame
- **WHEN** the MCP client receives malformed framing or invalid JSON from the server
- **THEN** the system SHALL treat the session as failed and SHALL NOT panic
