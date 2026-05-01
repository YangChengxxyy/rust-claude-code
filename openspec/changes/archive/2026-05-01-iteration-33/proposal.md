## Why

The CLI can execute tools with broad local access, which makes it risky to use in unfamiliar repositories or semi-trusted automation contexts. Iteration 33 establishes a safer execution boundary by combining OS sandboxing with an Auto permission mode that keeps routine work fast while still escalating risky operations.

## What Changes

- Add configurable sandbox execution for tool calls, with platform-specific isolation on macOS and Linux.
- Add sandbox network controls and allowed-path configuration.
- Add an Auto permission mode that automatically approves low-risk operations while running background safety checks.
- Add fallback behavior from Auto mode to confirmation when checks detect dangerous commands, invalid paths, or suspicious outputs.
- Add CLI and configuration wiring for sandbox options, Auto mode, and sandbox-aware Bash auto-approval.

## Capabilities

### New Capabilities

- `sandbox-execution`: Defines sandbox configuration, platform support, path and network restrictions, and sandboxed tool execution behavior.
- `auto-permission-mode`: Defines automatic approval behavior, safety checks, fallback confirmation, and integration with sandbox state.

### Modified Capabilities

- `settings-merge`: Adds sandbox configuration fields and their precedence with existing CLI, environment, project, and user configuration layers.

## Impact

- Affected crates: `core`, `cli`, and `tools`.
- Adds new runtime configuration for sandbox enablement, allowed paths, network policy, and Auto mode.
- Extends permission mode parsing and permission checks.
- Extends Bash and file-tool execution paths to account for sandbox and Auto-mode safety decisions.
- Adds platform-dependent behavior for macOS `sandbox-exec` and Linux `bubblewrap` or namespace isolation, with graceful unsupported-platform errors.
