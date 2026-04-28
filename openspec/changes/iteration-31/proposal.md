## Why

Hooks currently cover tool and prompt/stop integration, but they do not fire for session lifecycle events and cannot transform tool inputs before execution. Custom agents are also missing, which prevents users from defining reusable specialized sub-agents discoverable from project configuration.

## What Changes

- Add `SessionStart` and `SessionEnd` hook events that fire at the beginning and end of a CLI/TUI session.
- Extend hook results so `PreToolUse` hooks can return `updatedInput` to modify tool input before execution.
- Add hook `once` support so selected hooks execute at most once per session.
- Preserve real session identifiers through hook payloads and persisted session files so lifecycle, prompt, stop, and tool hooks can correlate events to one session.
- Add custom agent discovery from `.claude/agents/` definition files.
- Define custom agent metadata: `name`, `description`, `system_prompt`, tool allowlist, and optional model.
- Register discovered custom agents so the main agent can invoke them through `AgentTool` by name.
- Add an `/agents` slash command that lists available custom agents.
- Enhance `AgentTool` input and execution to support selecting a named custom agent.
- Preserve bounded nested sub-agent execution when AgentTool dispatches named custom agents instead of disabling deeper delegation.

## Capabilities

### New Capabilities
- `custom-agents`: Discovery, validation, listing, and execution contract for user-defined custom agents.

### Modified Capabilities
- `hook-config`: Add lifecycle hook events, `once` configuration, and `updatedInput` response support.
- `hook-execution`: Define execution semantics for lifecycle hooks, one-shot hooks, and input mutation.
- `hook-integration`: Integrate lifecycle hooks and updated tool input into QueryLoop and session startup/shutdown.
- `agent-tool`: Allow AgentTool to invoke a named custom agent with agent-defined prompt, tools, and optional model.
- `slash-command-extensions`: Add `/agents` command behavior for listing discovered custom agents.

## Impact

- Affects `core` hook/settings types, custom agent definition models, and configuration loading.
- Affects `cli` QueryLoop/session startup/shutdown, session persistence, slash command handling, and AgentTool context wiring.
- Affects `tools` AgentTool schema and execution behavior.
- Adds unit coverage for hook parsing/execution changes, custom agent loading, `/agents`, and named AgentTool dispatch.
