## Why

Iteration 42 closes two remaining Phase 4 gaps: Bash still executes without OS-level isolation, and the `sdk` crate exposes internal modules but not a stable embedding API. This change reduces damage from unsafe shell commands and makes the Rust agent loop usable as a library by third-party callers.

## What Changes

- Add configurable sandbox execution for Bash, with platform adapters for macOS `sandbox-exec`, Linux `bubblewrap`, and a safe no-op fallback when isolation is unavailable.
- Add CLI and config controls for enabling sandbox execution and disabling network access inside sandboxed commands.
- Thread sandbox configuration through `AppState` / tool execution so Bash can wrap its spawned command before execution.
- Replace the SDK crate's module-only public surface with stable `Session`, `SessionBuilder`, `ResponseStream`, and `ResponseEvent` APIs.
- Support SDK construction with model, credentials, system prompt, permission mode, tools, hooks/output integrations, and custom tool injection.
- Add a minimal SDK example that compiles independently and demonstrates `Session::builder().send("hello")` usage.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `sandbox-execution`: define how sandbox configuration, platform adapter selection, Bash wrapping, unavailable-adapter fallback, and network policy must behave.
- `sdk-foundation`: define the stable public API for building sessions, sending prompts, consuming response events, and registering custom tools.

## Impact

- Affected crates: `core` for sandbox configuration/state types, `tools` for Bash sandbox wrapping and platform adapters, `cli` for CLI/config wiring, and `sdk` for the public API surface.
- Affected APIs: public `rust-claude-sdk` exports become intentional and documented; `BashTool` gains sandbox-aware execution through existing tool context/state rather than changing tool input schema.
- Runtime dependencies: no new Rust crate dependency is required; sandboxing uses system commands when available (`sandbox-exec` on macOS, `bwrap` on Linux).
- Compatibility: sandboxing is opt-in by default, so existing command execution behavior remains unchanged unless enabled.
