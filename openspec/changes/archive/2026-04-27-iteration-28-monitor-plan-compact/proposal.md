## Why

Iteration 28 closes three gaps that limit complex agent workflows: long-running process monitoring, agent-driven plan mode transitions, and compaction context recovery. These capabilities matter now because the project has completed the core tool, permission, settings, memory, and streaming foundations needed to support multi-step engineering work without losing context or requiring manual mode switches.

## What Changes

- Add a `MonitorTool` that starts a long-running command, watches stdout/stderr for regex-matched output, returns relevant events to the agent, and manages process lifecycle with timeout and explicit stop support.
- Add `EnterPlanModeTool` and `ExitPlanModeTool` so the agent can switch into read-only planning mode and then restore the previous permission mode with a plan summary.
- Enhance compaction so compacted conversations automatically re-inject project `CLAUDE.md` context, summarize tools exposed by MCP servers that have been used in the session, and preserve recent permission decision context.
- Extend `/compact` to accept optional retention strategy arguments instead of always using a single implicit behavior.

## Capabilities

### New Capabilities
- `monitor-tool`: Background command monitoring with regex event extraction, timeout handling, and process lifecycle management.
- `plan-mode-tools`: Agent-callable tools for entering and exiting plan mode while preserving permission-mode state.

### Modified Capabilities
- `compaction-engine`: Re-inject project guidance, MCP tool context, and recent permission decisions after compaction.
- `compact-command`: Add optional retention strategy arguments for user-controlled compaction behavior.

## Impact

- `crates/tools`: New monitor and plan-mode tool implementations plus registration metadata.
- `crates/core`: Shared state updates for monitor process tracking and plan-mode transition state if needed.
- `crates/cli`: Query loop integration for monitor events, plan-mode transitions, compaction context restoration, and `/compact` argument parsing.
- `crates/tui`: Tool display and status handling for monitor events if the existing bridge requires new event variants.
- Tests: Unit tests for tool behavior and command parsing, plus workspace-level regression coverage.
