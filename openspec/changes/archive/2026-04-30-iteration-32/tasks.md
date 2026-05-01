## 1. Configuration Models

- [x] 1.1 Extend MCP server config types to support `stdio`, `sse`, and `http` variants with transport-specific fields.
- [x] 1.2 Update MCP settings deserialization and merge tests for remote server configs, unsupported transport warnings, and existing stdio behavior.
- [x] 1.3 Add provider configuration types for Anthropic, Bedrock, and Vertex in `core`.
- [x] 1.4 Implement provider precedence resolution for CLI, environment flags, settings/config, and Anthropic fallback.

## 2. MCP Remote Transports

- [x] 2.1 Introduce an MCP transport abstraction shared by stdio, SSE, and Streamable HTTP sessions.
- [x] 2.2 Adapt the existing stdio MCP client to use the transport abstraction without changing current behavior.
- [x] 2.3 Implement SSE transport connection, request dispatch, event parsing, timeout handling, and startup failure reporting.
- [x] 2.4 Implement Streamable HTTP transport request dispatch, response parsing, timeout handling, and HTTP error mapping.
- [x] 2.5 Add reconnect handling with exponential backoff for remote MCP transports and runtime metadata updates.

## 3. MCP Tool Integration

- [x] 3.1 Ensure remote MCP `tools/list` results register proxy tools using `mcp__<server_name>__<tool_name>` names.
- [x] 3.2 Ensure remote MCP `tools/call` execution returns successful results, server errors, disconnected errors, and timeout errors through the local tool interface.
- [x] 3.3 Update `/mcp` status output to include transport type and remote disconnected or reconnecting states.
- [x] 3.4 Add MCP tests covering remote tool registration, permission filtering compatibility, and `/mcp` remote status display.

## 4. Provider Routing

- [x] 4.1 Refactor API request creation behind provider-specific adapters while preserving the existing query-loop client contract.
- [x] 4.2 Keep Anthropic provider behavior compatible with existing authentication, base URL, streaming, and usage parsing tests.
- [x] 4.3 Implement Bedrock endpoint construction, AWS credential loading/signing integration, request mapping, response normalization, and missing-credential errors.
- [x] 4.4 Implement Vertex endpoint construction, GCP credential authentication integration, request mapping, response normalization, and missing-credential errors.
- [x] 4.5 Add `--provider` CLI parsing and wire provider resolution into runtime configuration.

## 5. Verification

- [x] 5.1 Add unit tests for provider precedence, conflicting environment flags, and provider-specific endpoint construction.
- [x] 5.2 Add unit tests for SSE and HTTP MCP transport parsing and error handling using mocked endpoints or transport fixtures.
- [x] 5.3 Run `cargo test --workspace` and fix regressions.
- [x] 5.4 Run targeted MCP/API tests for remote transports and provider routing.
