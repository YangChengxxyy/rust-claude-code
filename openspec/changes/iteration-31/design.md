## Context

The existing hook system supports configured command hooks for tool, prompt, stop, and notification events, and QueryLoop can invoke hooks around tool execution. AgentTool can spawn independent sub-agents with inherited runtime configuration and bounded recursion, but it only accepts ad hoc prompts and optional tool filters.

Iteration 31 extends these foundations in two directions: richer hook lifecycle/input control and user-defined custom agents. The design should keep the existing crate boundaries: shared models in `core`, orchestration in `cli`, and AgentTool execution behavior in `tools`.

## Goals / Non-Goals

**Goals:**
- Add `SessionStart` and `SessionEnd` hooks that run once at session boundaries in both non-interactive and TUI flows.
- Allow `PreToolUse` hooks to return `updatedInput` so a hook can rewrite tool parameters before permission-approved execution.
- Support a `once` hook flag that prevents repeated execution of the same configured hook within a session.
- Discover custom agent definitions from `.claude/agents/` and expose them through `/agents` and AgentTool.
- Let named custom agents supply default system prompt, allowed tools, description, and optional model override for spawned sub-agents.

**Non-Goals:**
- No marketplace, remote registry, or dynamic installation for agents.
- No support for non-command hook types.
- No inheritance of hook runners into sub-agents unless explicitly added by a future change.
- No interactive custom-agent editor; definitions are file-based.

## Decisions

1. Store hook lifecycle additions in existing hook types.

   `HookEvent` will add `SessionStart` and `SessionEnd`, and `HookConfig` will add `once: Option<bool>`. This preserves existing settings parsing and merge behavior while keeping lifecycle events available to the same `HookRunner`.

   Alternative considered: create a separate lifecycle callback system. Rejected because hooks already provide command execution, JSON stdin, timeouts, and settings merge semantics.

2. Track `once` execution state per session in `HookRunner`.

   `HookRunner` should hold lightweight session-local state keyed by a stable hook identity derived from event, group index, hook index, and command. When a hook with `once: true` succeeds or fails after being started, later matching invocations in the same session skip it.

   Alternative considered: mutate loaded hook config after execution. Rejected because shared config may be reused and mutation would be harder to reason about behind `Arc`.

3. Represent `updatedInput` as validated JSON object replacement.

   A `PreToolUse` hook may return `updatedInput` alongside an approve decision. The QueryLoop should replace the pending tool input with this object before executing the tool. If `updatedInput` is present but not an object or cannot be converted to the tool input representation, the hook result is treated as non-mutating and logs a warning.

   Alternative considered: support partial merge patches. Rejected for this iteration because whole-object replacement is simpler, easier to test, and matches tool schemas that already consume full JSON input.

4. Load custom agents into a registry during CLI initialization.

   A new `CustomAgentRegistry` in `core` should discover Markdown definition files under `.claude/agents/` in the current project. The CLI wires the registry into slash command handling and `AgentContext`; tools remain independent of filesystem discovery.

   Alternative considered: have AgentTool read `.claude/agents/` directly. Rejected because tool execution should receive resolved runtime context rather than performing project configuration discovery.

5. Use Markdown files with front matter for custom agent definitions.

   Each `*.md` file under `.claude/agents/` will contain YAML front matter for `name`, `description`, optional `tools`, and optional `model`; the Markdown body is the agent `system_prompt`. This aligns with common Claude Code custom-agent authoring patterns and keeps long prompts readable.

   Alternative considered: JSON-only definitions. Rejected because system prompts are multiline prose and Markdown is easier to maintain.

6. Extend AgentTool with optional `agent` selection.

    AgentTool input adds an optional `agent` string. When present, AgentTool resolves the custom agent by name and applies its system prompt, default allowed tools, description, and optional model. Explicit `allowed_tools` in the tool input further restricts the agent-defined tool allowlist rather than broadening it.

    Alternative considered: expose one generated tool per custom agent. Rejected for this iteration to avoid dynamic tool schema churn and keep the parent model interface stable.

7. Keep one stable session identifier in `AppState` and reuse it across hooks and persistence.

   Session lifecycle, prompt, stop, and tool hooks should all receive the same real `session_id`, and session saves during an active or resumed conversation should continue writing to that same logical session record. Storing the identifier in `AppState.session` keeps hook execution and persistence aligned without introducing a second session-tracking object.

   Alternative considered: generate hook-only IDs independently from persisted session IDs. Rejected because it would make hook correlation and resumed-session auditing inconsistent.

8. Preserve recursive AgentTool execution by reusing the same sub-agent runner closure.

   Named custom agents should not disable deeper AgentTool delegation. The runner passed into spawned sub-agents should therefore recurse through the same bounded runner implementation, while depth checks remain enforced by AgentTool itself.

   Alternative considered: disable nested delegation for named agents only. Rejected because it creates surprising behavior differences between ad hoc and named sub-agents.

## Risks / Trade-offs

- Custom agent file format ambiguity -> Mitigate with strict validation, clear errors, and tests for missing/invalid fields.
- Hook `updatedInput` can create surprising behavior after permission approval -> Mitigate by applying hooks after permission approval as existing PreToolUse behavior does, and by surfacing mutation in debug/log output where available.
- `once` state in a shared runner can be hard to test under concurrency -> Mitigate with mutex-protected state and deterministic unit tests for repeated and parallel invocations.
- Named agents can recursively invoke AgentTool -> Mitigate by preserving the existing recursion depth limit and max-round bounds.
- Session lifecycle hooks may be skipped on error paths -> Mitigate by running `SessionEnd` from unified success/error cleanup points in both print and TUI flows.
- TUI and print flows can diverge on lifecycle hooks -> Mitigate by wiring lifecycle hooks at shared session/query entry points rather than only in UI code.

## Migration Plan

- Existing hook settings continue to parse because new fields and events are additive.
- Existing AgentTool calls continue to work when `agent` is omitted.
- Projects without `.claude/agents/` behave unchanged and `/agents` reports no custom agents.
- Rollback consists of removing the new registry wiring and ignoring the additive hook fields/events; no persisted data migration is required.

## Open Questions

- Should project-level `.claude/agents/` be the only discovery location for this iteration, or should user-level agents be added later?
- Should `updatedInput` mutation be reported to the model as part of the tool result or only affect execution?
