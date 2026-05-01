## Why

Rust Claude Code currently supports MCP only through local stdio servers and sends model requests only through the Anthropic API shape. Iteration 32 expands the integration surface so users can connect remote MCP servers and route Claude requests through managed cloud providers.

## What Changes

- Add MCP SSE and Streamable HTTP transport support for remote MCP servers.
- Extend MCP settings so each server can select `stdio`, `sse`, or `http` transport using its `type` field.
- Add reconnect and transient failure handling for remote MCP transports.
- Add provider configuration for Anthropic, Amazon Bedrock, and Google Vertex AI.
- Add CLI and environment selection for Bedrock and Vertex without changing tool or permission behavior.
- Add provider-specific endpoint construction and authentication/signing paths.

## Capabilities

### New Capabilities
- `mcp-remote-transports`: Remote MCP SSE and Streamable HTTP connections, including initialization, tool listing, tool calls, and reconnect behavior.
- `provider-routing`: Model provider selection and request routing across Anthropic, Amazon Bedrock, and Google Vertex AI.

### Modified Capabilities
- `mcp-config`: MCP server configuration expands beyond stdio to include remote SSE and HTTP server definitions.
- `mcp-tool-integration`: MCP proxy tools remain registered, filtered, permission-checked, and callable regardless of whether the backing server uses stdio or a remote transport.

## Impact

- `core` settings/config types gain provider and remote MCP transport configuration fields.
- `api` gains provider selection, provider-specific endpoint construction, and Bedrock/Vertex authentication support.
- MCP runtime/client code gains SSE and Streamable HTTP transport implementations with reconnect handling.
- `cli` gains provider selection through `--provider` and provider-related environment variables.
- Existing tool registry, permission checks, and query loop behavior must remain provider- and transport-agnostic.
