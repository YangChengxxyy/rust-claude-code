## Context

The codebase already has a mature split between `core`, `api`, `tools`, `sdk`, `cli`, and `tui`. The remaining iteration 42 gaps are concentrated in two places:

- `BashTool` builds a `tokio::process::Command` directly and executes it with the current session CWD, timeout handling, output truncation, and persistent `cd` behavior, but no sandbox boundary.
- `crates/sdk/src/lib.rs` only re-exports internal modules. The reusable agent loop exists in `sdk::agent_loop::QueryLoop`, but consumers still need to assemble low-level state, clients, tools, output sinks, and permissions themselves.

Existing specs already define baseline `sandbox-execution` and `sdk-foundation` capabilities. This change updates those contracts to match the current architecture and make them implementation-ready.

## Goals / Non-Goals

**Goals:**

- Make Bash sandboxing opt-in and explicit, preserving current behavior when disabled.
- Keep sandbox configuration in `core` so `cli`, `sdk`, and tools share the same contract.
- Implement platform adapters without adding Rust crate dependencies: macOS via `sandbox-exec`, Linux via `bwrap`, and unsupported platforms via a reporting adapter.
- Ensure `--sandbox-no-network` only affects sandboxed tools, not model API calls, MCP connections, or other host-side runtime services.
- Provide a stable SDK API centered on `Session::builder()`, `SessionBuilder`, `Session::send()`, `Session::send_with_tools()`, `ResponseStream`, and `ResponseEvent`.
- Allow SDK users to inject custom tools without depending on CLI/TUI internals.

**Non-Goals:**

- Full containerization or VM isolation.
- Sandboxing non-Bash tools in this iteration.
- Windows sandbox support beyond an explicit unsupported adapter.
- A full TypeScript/Python SDK; this iteration only creates the Rust public API.
- Backward-compatible stabilization of every internal `sdk` module; only the new public surface is intended to be stable.

## Decisions

### Decision: Put sandbox data types in `core`

`SandboxConfig`, `NetworkPolicy`, and adapter-facing command policy types belong in `core` because configuration loading, `AppState`, and permission decisions already live there. `tools` can consume the config through `ToolContext.app_state` without changing the Bash tool input schema.

Alternative considered: keep sandbox config in `tools`. That would make CLI configuration and SDK construction depend on tools-specific types and would duplicate state plumbing.

### Decision: Sandbox is opt-in and fail-closed when requested but unavailable

When `sandbox.enabled` is false, Bash remains unsandboxed. When it is true and no platform adapter is available, Bash returns a clear sandbox unsupported error instead of silently running without isolation.

Alternative considered: no-op fallback that logs a warning and runs unsandboxed. That is easier operationally but unsafe because users enabling `--sandbox` reasonably expect isolation.

### Decision: Wrap the spawned shell rather than rewriting commands

`BashTool` should keep its existing `sh -c` command construction, final-CWD marker, timeout, and output behavior. The sandbox adapter receives the already-constructed command and transforms it into a sandbox launcher invocation while preserving args, current_dir, and environment as much as the platform allows.

Alternative considered: parse and restrict shell commands directly. That is brittle, incomplete, and duplicates permission logic.

### Decision: Use system sandbox tools only

macOS uses `sandbox-exec` with a generated temporary profile. Linux uses `bwrap` if present. No new Rust crate dependency is required for iteration 42.

Alternative considered: add a Rust sandboxing library. Cross-platform support is uneven and would still require platform-specific policy work.

### Decision: SDK Session owns high-level orchestration, not UI

`SessionBuilder` constructs an API client, tool registry, app state, and query loop configuration. UI integration stays behind the existing `OutputSink`, `PermissionUI`, and `UserQuestionUI` traits. Headless callers can omit UI traits and receive stream events from `ResponseStream`.

Alternative considered: expose `QueryLoop` as the primary public API. That leaks implementation details and still leaves consumers to build too much internal state.

### Decision: ResponseStream is event-oriented

The SDK stream yields `ResponseEvent` values for text, thinking, tool use, tool result, usage, errors, and completion. This mirrors how TUI and CLI already think about output and avoids exposing raw Anthropic SSE events.

Alternative considered: return only final `Message`. This is useful for `send()` but insufficient for embedded UIs that need token-level updates and tool progress.

## Risks / Trade-offs

- [Risk] macOS `sandbox-exec` is deprecated on some platform versions. → Mitigation: detect availability at runtime and return a clear unsupported error when enabled but unavailable.
- [Risk] Linux `bwrap` may not be installed or usable under restricted hosts. → Mitigation: runtime detection plus fail-closed behavior when sandbox is explicitly enabled.
- [Risk] Filesystem policy can break common shell commands that need `/bin`, `/usr`, dynamic libraries, or temp paths. → Mitigation: adapters must include minimal read-only system paths needed to launch shells while restricting user data access to configured allowed paths.
- [Risk] Network denial semantics differ between `sandbox-exec` and `bwrap`. → Mitigation: specs require testable behavior for outbound network attempts, not identical implementation mechanics.
- [Risk] SDK API may overfit current internals. → Mitigation: keep the public surface small and convert internal events into stable `ResponseEvent` values.
- [Risk] SessionBuilder can hide too much configuration. → Mitigation: provide explicit builder methods for core options and custom tools while preserving sensible defaults.

## Migration Plan

- Add sandbox config/state types with defaults that preserve current unsandboxed behavior.
- Add CLI/config parsing for sandbox options without enabling them by default.
- Implement adapters and wire Bash through the adapter only when sandboxing is enabled.
- Add the SDK public API alongside existing modules, then re-export the intended stable types from `sdk/src/lib.rs`.
- Add examples and tests before relying on the new API from external callers.

Rollback is straightforward because sandboxing is opt-in and SDK additions are additive: disable `--sandbox` / config `sandbox.enabled`, or stop exporting the new SDK API before release if tests expose a design issue.

## Open Questions

- What exact default allowed paths should be included beyond project root and configuration/session directories? The implementation should start minimal and add only paths required for common shell startup.
- Should SDK `SessionBuilder::api_key()` accept bearer tokens and provider routing fields directly, or should it primarily accept a resolved `Config`? The first implementation can support both if minimal.
