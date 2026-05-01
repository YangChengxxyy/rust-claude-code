## Context

The current implementation has an Anthropic-focused API client and MCP integration centered on local stdio servers. Existing MCP capabilities already define server configuration, stdio startup, tool discovery, tool proxy registration, permission checks, and `/mcp` status display.

Iteration 32 extends two integration boundaries: MCP transports must support remote servers over SSE and Streamable HTTP, and model requests must be routable through Anthropic, Amazon Bedrock, or Google Vertex AI. These changes cross `core`, `api`, `cli`, and MCP runtime code, while preserving the existing query loop, tool registry, and permission contracts.

## Goals / Non-Goals

**Goals:**

- Add remote MCP transport support for SSE and Streamable HTTP servers.
- Select MCP transport from each configured server's `type` field.
- Reuse existing MCP initialization, `tools/list`, `tools/call`, metadata, and proxy-tool behavior across transports.
- Add provider selection for Anthropic, Bedrock, and Vertex through config, environment, and CLI.
- Keep provider routing isolated behind the existing model-client boundary so tools and permissions remain unchanged.

**Non-Goals:**

- Supporting arbitrary non-Claude model APIs.
- Implementing OAuth or interactive cloud credential setup.
- Adding a full MCP server management UI beyond existing `/mcp` status surfaces.
- Changing permission semantics for MCP tools.
- Implementing sandbox or auto mode behavior planned for later iterations.

## Decisions

### Transport abstraction for MCP

Introduce an MCP transport abstraction with implementations for stdio, SSE, and Streamable HTTP. The MCP session layer should issue JSON-RPC `initialize`, `tools/list`, and `tools/call` through this abstraction rather than owning transport-specific I/O directly.

Rationale: This keeps protocol behavior shared and avoids duplicating request correlation, timeout handling, and error conversion for each transport.

Alternative considered: implement separate stdio, SSE, and HTTP clients end-to-end. That would be faster to patch but risks divergent behavior and test duplication.

### Configuration-driven transport selection

Extend `mcpServers` so `type: "stdio"`, `type: "sse"`, and `type: "http"` choose the transport. Stdio retains `command`, `args`, `env`, and `cwd`; remote transports use `url` plus optional headers and timeout/reconnect fields.

Rationale: This follows the existing config shape and makes remote transport selection explicit and deterministic.

Alternative considered: infer transport from `command` or URL fields. Inference would reduce config verbosity but creates ambiguous validation and poor error messages.

### Remote reconnect is transport-local

Implement exponential backoff reconnect handling inside remote transport/session management. A disconnected remote server should update MCP runtime metadata and fail in-flight tool calls with clear local tool errors, while future calls can use a reconnected session when available.

Rationale: Remote MCP servers have network failure modes that stdio does not. Keeping reconnect in MCP runtime avoids leaking transport state into `ToolRegistry` or `QueryLoop`.

Alternative considered: reconnect only on startup. That is simpler but fails long-running TUI sessions and remote MCP workflows after transient network interruptions.

### Provider enum and request adapter boundary

Add a provider config enum in `core` and route API requests in `api` through provider-specific adapters. Anthropic keeps the current direct request path. Bedrock and Vertex adapters construct provider-specific endpoints and authentication/signing while preserving the logical message request/stream events consumed by `QueryLoop`.

Rationale: The query loop should not know which provider is used. It should continue to receive the same logical stream events and usage data.

Alternative considered: create separate CLI query-loop clients per provider. That would make provider behavior visible too high in the stack and increase the risk of tool-loop regressions.

### Provider selection precedence

Use explicit CLI selection first, then provider environment flags, then settings/config defaults, with Anthropic as the fallback. `CLAUDE_CODE_USE_BEDROCK=1` and `CLAUDE_CODE_USE_VERTEX=1` select cloud providers; `--provider` gives an explicit override.

Rationale: This matches the iteration plan and keeps provider choice predictable for scripts.

Alternative considered: infer provider from credentials. Credential inference is fragile because developers may have multiple cloud credentials available simultaneously.

## Risks / Trade-offs

- Remote MCP streaming semantics differ across servers -> Validate both SSE and Streamable HTTP framing with unit tests around parsed JSON-RPC responses and connection lifecycle.
- Reconnect can duplicate initialization or tool discovery state -> Replace server runtime metadata atomically after a successful reconnect and keep failed state visible until then.
- Bedrock and Vertex authentication add dependency and environment complexity -> Isolate signing/auth code behind provider adapters and add tests for endpoint/request construction without requiring live credentials.
- Provider-specific streaming payloads may not map perfectly to existing stream events -> Normalize at the adapter boundary and keep the `ModelClient` contract stable.
- Multiple provider selection inputs can conflict -> Define deterministic precedence and surface a clear configuration error for mutually exclusive environment flags.

## Migration Plan

- Existing Anthropic and stdio MCP configurations continue to work without changes.
- Existing `type: "stdio"` MCP servers retain current fields and behavior.
- Configurations that previously warned on `type: "sse"` should begin loading as remote MCP server definitions when required fields are present.
- Rollback can disable remote MCP configs or select `--provider anthropic` / unset cloud provider environment flags.

## Open Questions

- Whether remote MCP headers should support environment-variable interpolation in this iteration or only literal values.
- Whether `http` should be named `streamable-http` in user-facing config for closer MCP terminology, or keep the shorter `http` from the iteration plan.
- Whether Bedrock/Vertex streaming should be required for first delivery or whether non-streaming provider calls are acceptable as an incremental fallback.
