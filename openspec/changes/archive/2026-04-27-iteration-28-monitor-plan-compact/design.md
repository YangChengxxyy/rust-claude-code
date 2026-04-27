## Context

Iteration 28 builds on the completed tool registry, permission manager, TUI bridge, session state, MCP stdio integration, and compaction engine. The current project can run short-lived tools and compact long conversations, but it lacks first-class support for long-running background commands, agent-controlled plan-mode transitions, and context restoration after compaction.

The most relevant constraints are:

- Tool execution already flows through `ToolRegistry`, `ToolContext`, and `QueryLoop` permission checks.
- Permission mode lives in shared `AppState`, so plan-mode tools should update that state rather than introduce a parallel mode store.
- Bash is already the command execution boundary; monitor should reuse the same safety expectations instead of bypassing permissions.
- Compaction already replaces message history with a summary plus preserved messages; this iteration should enrich the summary context, not replace the compaction architecture.

## Goals / Non-Goals

**Goals:**

- Add a `MonitorTool` for long-running commands that returns regex-matched stdout/stderr lines and lifecycle status.
- Keep monitor execution bounded through explicit timeout handling and process cleanup.
- Add `EnterPlanModeTool` and `ExitPlanModeTool` so an agent can temporarily switch to read-only planning and then restore the previous mode.
- Preserve enough context through compaction to keep project instructions, recently used MCP tools, and recent permission decisions available after history replacement.
- Extend `/compact` with explicit retention strategies while keeping default behavior unchanged.

**Non-Goals:**

- A general daemon supervisor or persistent process manager across application restarts.
- Streaming every monitor output line into the UI; the monitor returns selected events based on a regex pattern.
- Full sandboxing for monitored commands.
- Changing the semantics of existing permission modes beyond temporary plan-mode transitions.
- Rewriting compaction prompts or token estimation from scratch.

## Decisions

### 1. Monitor as a bounded tool invocation

`MonitorTool` will run as a normal tool call that starts a child process, reads stdout/stderr concurrently, records matching lines, and returns when the process exits, the timeout expires, or enough matched events have been collected.

This keeps the first implementation compatible with the existing request/response tool contract. It also avoids introducing a cross-turn background event bus before the rest of the system needs one.

Alternative considered: spawn persistent background monitors that outlive the tool call and push events into later agent turns. Rejected for this iteration because it requires durable monitor IDs, cancellation commands, session persistence, and query-loop wakeup semantics. The spec still requires process cleanup and timeout behavior, but not restart persistence.

### 2. Monitor command permission follows Bash policy

Monitor executes shell commands and therefore should be permission-checked using the same command classification as Bash. The implementation can expose a distinct tool name for allow/deny rules, but the command string itself must still go through the existing permission manager path before execution.

Alternative considered: treat monitor as read-only because it observes output. Rejected because the monitored command may mutate files, start services, or access the network.

### 3. Plan-mode tools mutate shared permission state

`EnterPlanModeTool` stores the current permission mode in `AppState` and sets mode to `Plan`. `ExitPlanModeTool` requires a plan summary, restores the saved mode, and clears the temporary saved value. If there is no saved mode, exit is a no-op with a clear tool result.

Alternative considered: have the query loop intercept these tool names and mutate mode outside the tool system. Rejected because it creates special cases and makes tool tests harder. The tools should use the same `ToolContext` state access pattern as existing stateful tools.

### 4. Compaction context enrichment as prompt input

Compaction restoration should be added by enriching the compaction prompt and compacted summary message with supplemental context: current project guidance, MCP tools used in the session, and recent permission decisions. The message-history replacement model remains summary plus preserved messages.

Alternative considered: inject extra synthetic system messages after compaction. Rejected because the current message model and Anthropic request builder already have a dedicated system prompt path; adding synthetic user messages risks confusing conversation chronology.

### 5. `/compact` strategies are explicit but conservative

The slash command will support strategy names such as `default`, `aggressive`, and `preserve-recent`. Unknown strategies return a user-facing error. With no argument, `/compact` keeps current default behavior.

Alternative considered: expose raw numeric thresholds in the command. Rejected for now because named strategies are easier to test, document, and keep stable.

## Risks / Trade-offs

- [Monitor commands can hang or produce unbounded output] -> Enforce timeout, cap captured output, and kill the child process on timeout.
- [Regex filtering may hide useful context] -> Include process exit status and a count of omitted non-matching lines in the result.
- [Plan-mode restore can become stale if the user manually changes mode while in plan mode] -> Store one previous mode and make exit behavior deterministic; manual mode changes can overwrite current mode but should not panic.
- [Compaction prompt bloat can reduce savings] -> Cap injected project guidance, MCP tool summary, and permission context to small bounded sections.
- [MCP tool usage tracking may not exist centrally] -> Start with a best-effort session-local list from tool execution events; omit the section if no data is available.
