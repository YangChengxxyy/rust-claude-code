## 1. Sandbox Configuration

- [x] 1.1 Add `core::sandbox` types for `SandboxConfig`, `NetworkPolicy`, adapter availability, defaults, and path canonicalization helpers.
- [x] 1.2 Add sandbox configuration to `Config`, config overrides/provenance, raw config serialization, and `AppState`/session state defaults.
- [x] 1.3 Add CLI flags `--sandbox` and `--sandbox-no-network`, wire them into config resolution, and preserve disabled-by-default behavior.
- [x] 1.4 Add unit tests for sandbox config defaults, CLI/config override precedence, and allowed path resolution.

## 2. Sandbox Adapters And Bash Integration

- [x] 2.1 Add `tools::sandbox` adapter modules for macOS `sandbox-exec`, Linux `bwrap`, and unsupported platforms.
- [x] 2.2 Implement runtime adapter availability checks and fail-closed errors when sandboxing is enabled but no adapter is available.
- [x] 2.3 Implement command wrapping while preserving BashTool shell command construction, current directory, timeout, output truncation, and final-CWD capture.
- [x] 2.4 Implement filesystem allow-path policy for macOS profiles and Linux bind arguments, including minimal system paths required to launch the shell.
- [x] 2.5 Implement network policy mapping for allow and deny modes in the platform adapters.
- [x] 2.6 Add BashTool tests for disabled sandbox preservation, enabled sandbox wrapping, unsupported-runtime failure, and final-CWD behavior under sandbox wrapping.

## 3. SDK Public API

- [x] 3.1 Add `sdk::session` module with `Session`, `SessionBuilder`, SDK `Error`/`Result`, and `Session::builder()` exports.
- [x] 3.2 Implement builder methods for credentials/config, model, base URL, system prompt, permission mode, max rounds, output sink, permission UI, user question UI, hooks, compaction config, explicit client, and explicit tool registry.
- [x] 3.3 Build default headless sessions from minimal configuration using the existing API client, default tool registry, app state, and `QueryLoop`.
- [x] 3.4 Add custom tool injection with duplicate-name detection and a clear build error.
- [x] 3.5 Re-export only the intended stable SDK entry points from `sdk/src/lib.rs` while keeping internal modules available as needed for existing crate usage.

## 4. SDK Response Streaming

- [x] 4.1 Add `ResponseEvent` variants for text delta, thinking delta, tool use, tool result, usage, error, and done.
- [x] 4.2 Add `ResponseStream` implementing `Stream<Item = ResponseEvent>` using an internal channel-backed `OutputSink` bridge.
- [x] 4.3 Implement `Session::send(&str)` to run the agent loop and return the final assistant `Message`.
- [x] 4.4 Implement `Session::send_streaming(&str)` to return live response events and complete with `ResponseEvent::Done`.
- [x] 4.5 Implement `Session::send_with_tools(&str, Vec<ToolResult>)` to incorporate externally supplied tool results before continuing the session turn.

## 5. Examples And Verification

- [x] 5.1 Add `crates/sdk/examples/sdk_basic.rs` demonstrating `Session::builder()`, prompt sending, and response consumption without CLI/TUI dependencies.
- [x] 5.2 Add SDK tests for minimum builder construction, full builder configuration, explicit component injection, streaming event conversion, and duplicate custom tool rejection.
- [x] 5.3 Add platform-gated sandbox tests for macOS `sandbox-exec` and Linux `bwrap` when available, with skipped/fallback assertions when unavailable.
- [x] 5.4 Run `cargo fmt --all`.
- [x] 5.5 Run `cargo test -p rust-claude-core`, `cargo test -p rust-claude-tools`, and `cargo test -p rust-claude-sdk`.
- [x] 5.6 Run `cargo build -p rust-claude-sdk --example sdk_basic`.
- [x] 5.7 Run `cargo test --workspace`.
