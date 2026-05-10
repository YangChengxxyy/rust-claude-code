## Why

The current agent loop waits for the full assistant response before executing any tool calls, so multi-tool responses pay unnecessary latency even when early tool-use blocks are already complete. Phase 4 iteration 38 targets this performance gap by starting eligible tools during response streaming while preserving the existing permission, hook, ordering, and fallback guarantees.

## What Changes

- Add a streaming tool execution path that starts completed `tool_use` blocks before `message_stop` when streaming responses are enabled.
- Preserve result ordering by returning tool results in the same order that tool-use blocks appeared in the assistant response.
- Respect the existing tool concurrency model: concurrency-safe tools may run in parallel, while non-concurrency-safe tools are serialized behind prior non-concurrency-safe work.
- Add cancellation handling so user interrupts discard pending streaming execution and cancel interruptible tools.
- Add Bash sibling-failure behavior so failed Bash executions can cancel other in-flight Bash commands from the same assistant turn without canceling unrelated read-only tools.
- Keep a safe fallback to the existing collect-then-execute path if streaming execution cannot be completed reliably.

## Capabilities

### New Capabilities

- `streaming-tool-execution`: Starts and manages tool execution as tool-use blocks complete during assistant response streaming.
- `tool-interrupt-behavior`: Defines how tools participate in user interruption and sibling cancellation during streaming execution.

### Modified Capabilities

None.

## Impact

- `sdk` crate: adds a streaming tool executor and integrates it into the agent-loop streaming path.
- `tools` crate: extends the `Tool` trait with interrupt behavior metadata and applies defaults to existing tools.
- `core` crate: may add shared interrupt/cancellation enums if keeping the trait contract outside `tools` is cleaner.
- Tests: adds mock streaming tests for early execution, ordering, serialization, cancellation, failure fallback, and Bash sibling cancellation.
- Dependencies: likely adds `tokio-util` for `CancellationToken`.
