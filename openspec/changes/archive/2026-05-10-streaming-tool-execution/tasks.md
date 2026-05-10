## 1. Tool Interrupt Contract

- [x] 1.1 Add an `InterruptBehavior` enum with `Cancel` and `Block` variants in the crate that owns the tool execution contract.
- [x] 1.2 Extend the `Tool` trait with `interrupt_behavior(&self) -> InterruptBehavior` defaulting to `Cancel`.
- [x] 1.3 Update existing tool implementations and tests to compile with the new trait method.
- [x] 1.4 Add tests proving default tool interrupt behavior is cancelable and explicit blocking behavior is observable.

## 2. Streaming Executor Core

- [x] 2.1 Add the `tokio-util` dependency needed for `CancellationToken`.
- [x] 2.2 Create a turn-scoped `StreamingToolExecutor` module that accepts complete `tool_use` content blocks with sequence numbers.
- [x] 2.3 Implement immediate scheduling for concurrency-safe tools and a serial lane for non-concurrency-safe tools.
- [x] 2.4 Buffer completed results and return them from `finish()` in original tool-use order.
- [x] 2.5 Implement `discard()` so pending work is canceled or awaited according to each tool's interrupt behavior.
- [x] 2.6 Implement same-turn Bash sibling cancellation without canceling non-Bash tools.

## 3. Shared Execution Semantics

- [x] 3.1 Reuse or extract existing permission-check logic so streamed execution denies tools the same way as collect-then-execute.
- [x] 3.2 Reuse or extract existing hook-wrapped tool invocation so streamed execution emits matching hook events.
- [x] 3.3 Ensure streamed denied, failed, canceled, and successful tools all produce model-compatible `tool_result` entries.
- [x] 3.4 Add unit tests for permission denial, hook execution, ordered results, safe-tool parallelism, unsafe-tool serialization, and Bash sibling cancellation.

## 4. Agent Loop Integration

- [x] 4.1 Update the streaming response loop to notify `StreamingToolExecutor` when a `tool_use` block is complete.
- [x] 4.2 Ensure incomplete streamed tool input is never scheduled before the block stop event and complete JSON input is available.
- [x] 4.3 Call `finish()` after assistant `message_stop` and append ordered tool results before the next model round.
- [x] 4.4 Wire user interrupt handling to `discard()` for streamed tool execution.
- [x] 4.5 Keep the existing collect-then-execute path for non-streaming responses and fallback cases.

## 5. Fallback And Verification

- [x] 5.1 Implement fallback from streaming execution to collect-then-execute when executor setup fails before committed results and the assistant message can be reconstructed.
- [x] 5.2 Surface an error instead of falling back when the assistant message cannot be reconstructed safely.
- [x] 5.3 Add mock-stream agent-loop tests proving the first tool can start before `message_stop` and result order remains stable when later tools finish first.
- [x] 5.4 Add interrupt tests proving cancelable tools are canceled and blocking tools finish before discard completes.
- [x] 5.5 Run `cargo test --workspace` and fix any failures.
- [x] 5.6 Run `cargo check --workspace` after tests pass.
