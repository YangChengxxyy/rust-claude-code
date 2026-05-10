## Context

The current streaming response path assembles a full assistant message, waits for `message_stop`, detects tool-use blocks, and then calls the existing batch executor. This preserves correctness but delays tool execution until the slowest tool-use block and any trailing assistant output have finished streaming.

The existing tool system already exposes the metadata needed for a staged executor: `is_concurrency_safe()` separates tools that can run in parallel from tools that must be serialized, permission checks run before execution, and hook integration already wraps tool execution. The new design should reuse those contracts rather than adding a parallel execution model.

## Goals / Non-Goals

**Goals:**

- Start executing a tool as soon as its streamed `tool_use` block is complete and approved.
- Preserve the observable assistant message and tool-result order expected by the model.
- Reuse the existing permission, hook, app-state, and tool registry behavior.
- Support cancellation on user interrupt and sibling cancellation for Bash tools in the same assistant turn.
- Fall back to collect-then-execute if streaming execution cannot safely finish.

**Non-Goals:**

- Changing tool schemas or adding new user-facing tools.
- Speculative execution before a `tool_use` input JSON object is complete.
- Executing non-streaming API responses through the streaming executor.
- Reworking TUI rendering of tool progress beyond existing bridge events.

## Decisions

### Add a Turn-Scoped StreamingToolExecutor

Create a `StreamingToolExecutor` owned by one assistant turn. It accepts complete `tool_use` content blocks as the stream emits block-stop events, schedules eligible tools immediately, and returns all results from `finish()` in original block order.

Alternatives considered:

- Execute directly inside the stream loop. This makes ordering, cancellation, and fallback state harder to reason about.
- Extend the existing batch executor only. This keeps one code path but cannot start tools before the full response is collected without adding equivalent state internally.

### Keep Ordering Separate From Completion

Each accepted tool receives a monotonically increasing sequence number. Spawned tasks can complete in any order, but `finish()` sorts or drains by sequence before producing tool results for the next model request.

Alternatives considered:

- Return results in completion order. This is faster to surface but changes the model-visible order and risks mismatching tool-use IDs with expected conversational flow.

### Serialize Only Non-Concurrency-Safe Tools

Concurrency-safe tools can start immediately. Non-concurrency-safe tools run through a single serial lane that preserves their relative order while allowing already-safe read-like work to overlap.

Alternatives considered:

- Serialize all tools. This is simpler but loses much of the latency benefit.
- Run all tools in parallel. This can violate existing safety assumptions for writes, edits, and stateful tools.

### Add Tool Interrupt Behavior Metadata

Extend the `Tool` trait with `interrupt_behavior() -> InterruptBehavior`, defaulting to `Cancel`. Tools that must not be aborted mid-operation can return `Block`, causing user interrupts to wait for completion while preventing new work from starting.

Alternatives considered:

- Hard-code behavior by tool name in the executor. This couples scheduler policy to individual tool implementations and makes new tools easy to misclassify.

### Use CancellationToken For Turn Cancellation

Use `tokio_util::sync::CancellationToken` for a shared turn token and separate Bash sibling token. `discard()` cancels the turn token and awaits or drops task handles according to interrupt behavior. Bash execution failure cancels only the Bash sibling token for that assistant turn.

Alternatives considered:

- Use ad-hoc atomic flags. This avoids a dependency but makes async propagation and child task coordination more error-prone.

### Fallback On Streaming Executor Failure

If scheduling, permission/hook integration, or stream consumption fails in a way that leaves the executor state uncertain, discard in-flight streaming execution and use the existing collect-then-execute path when the full assistant message can still be reconstructed. If reconstruction is impossible, surface the underlying error.

Alternatives considered:

- Fail immediately on any executor error. This preserves strictness but regresses recoverability compared with the current path.

## Risks / Trade-offs

- Duplicate execution during fallback -> Mitigation: only fallback before any result has been committed to the next request, and discard/cancel in-flight tasks first.
- More concurrency can expose tool bugs -> Mitigation: honor `is_concurrency_safe()` and keep unsafe tools on a serial lane.
- User interrupts can leave child processes running -> Mitigation: propagate cancellation tokens through Bash and require process cleanup in Bash tests.
- Streaming and batch paths may diverge -> Mitigation: keep permission, hook, and tool invocation logic shared behind helper functions where possible.
- Result buffering can hide early failures until `finish()` -> Mitigation: record errors immediately, cancel affected sibling work when required, and return ordered error results at finish.

## Migration Plan

Implement the streaming executor behind the existing streaming path with no configuration migration. Existing non-streaming and fallback execution remain available, so rollback is removing or disabling the streaming executor integration and returning to collect-then-execute.

Add unit tests around the executor before wiring it into the agent loop, then add agent-loop mock stream tests that prove tools start before `message_stop` and results remain ordered.

## Open Questions

- Which existing tools, if any, should be marked `InterruptBehavior::Block` initially?
- Should Bash sibling cancellation apply to all Bash failures or only interruption/error classes that indicate the turn can no longer proceed safely?
