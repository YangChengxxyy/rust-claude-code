## Context

Slash commands are currently represented by a static `SLASH_COMMANDS` slice in the TUI app and dispatched through a large `match` in `handle_slash_command`. That approach works for the current command set, but iteration 30 adds a larger command batch with more session-state mutations and makes continued static-table growth increasingly error-prone.

The existing TUI already routes user intents to the CLI worker through `UserCommand`, displays system output via `AppEvent`, and has tests around command parsing and suggestion rendering. The change should preserve that split: the TUI owns input parsing and local UI state, while operations that need runtime/session/config context are sent to the background worker.

## Goals / Non-Goals

**Goals:**

- Register built-in slash commands through a single command registry used by help output, suggestions, validation, and dispatch metadata.
- Add the iteration 30 commands: `/plan`, `/rename`, `/branch`, `/recap`, `/rewind`, `/add-dir`, `/login`, `/logout`, `/effort`, and `/keybindings`.
- Keep local-only commands local when possible, and route commands needing query-loop or shared state changes through `UserCommand`.
- Make `/help` and slash suggestions reflect the registry without maintaining duplicate command lists.
- Add tests for command registration, command routing, and stateful command behavior.

**Non-Goals:**

- Full Anthropic OAuth or browser-based authentication flows.
- Persistent multi-branch session storage beyond creating an independent in-memory session branch from current messages.
- A complete multi-root workspace permission model beyond tracking additional directories for session context and tool/prompt awareness.
- Replacing all TUI event handling with a plugin system.

## Decisions

- Introduce a TUI command registry as the command source of truth.
  - Rationale: the existing static slice duplicates data with dispatch logic and suggestions. A registry with command definitions, aliases if needed, usage, description, and handler kind lets `/help`, suggestions, validation, and tests read the same metadata.
  - Alternative considered: append more entries to `SLASH_COMMANDS` and extend the current `match`. This is smaller initially but keeps scaling problems and makes the iteration 30 registry acceptance criterion unmet.

- Keep dispatch implementation explicit while registry-driven metadata controls visibility and validation.
  - Rationale: the current codebase uses direct `UserCommand` variants and local state updates. Keeping handlers explicit avoids premature dynamic trait/plugin complexity while still removing duplicated command metadata.
  - Alternative considered: store boxed async command handlers in the registry. That adds lifetime and async trait complexity with limited benefit for first-party commands.

- Route durable or cross-component effects through `UserCommand`.
  - Rationale: commands such as `/plan`, `/rename`, `/branch`, `/recap`, `/rewind`, `/add-dir`, `/login`, `/logout`, and `/effort` need either query-loop state, shared `AppState`, session metadata, or worker output. Sending typed commands keeps TUI parsing testable and leaves operational behavior in the CLI worker.
  - Alternative considered: mutate everything in the TUI. This would duplicate session/query-loop state and break commands that need model or credential context.

- Treat `/login` and `/logout` as basic credential-management helpers.
  - Rationale: the current project resolves credentials from config, environment, `ANTHROPIC_AUTH_TOKEN`, and `apiKeyHelper`. Iteration 30 can provide user-visible account status, setup guidance, and local config cleanup without inventing a full OAuth subsystem.
  - Alternative considered: implement upstream-equivalent login. That is larger, provider-specific, and not required by the iteration acceptance criteria.

- Implement `/effort` as a runtime effort setting that maps to model thinking budget when supported.
  - Rationale: existing extended-thinking support can use a small enum-like setting (`low`, `medium`, `high`) to influence request construction without exposing raw token budgets in the TUI command contract.
  - Alternative considered: make users set numeric thinking budgets directly. That is less ergonomic and does not match the planned command surface.

## Risks / Trade-offs

- Registry refactor could regress existing command availability -> keep compatibility tests for current commands and add a test that `/help`, suggestions, and validation all come from the registry.
- `/rewind` can desynchronize visible TUI messages from query-loop history -> implement the history mutation in the same component that owns conversation context and send a confirmation event back to the TUI.
- `/branch` semantics can be overbuilt -> define it as an in-memory fork of current message history with a new branch/session identity, and avoid persistent branch storage unless existing session persistence makes it straightforward.
- `/login` and `/logout` may imply full account auth to users -> output explicit messages describing supported config/env/apiKeyHelper behavior and whether persistent changes were made.
- `/add-dir` can create unsafe path assumptions -> canonicalize paths, reject missing paths, and show the active extra directory list after updates.

## Migration Plan

1. Add the registry while preserving all existing command names and usage strings.
2. Move help and suggestion generation to the registry.
3. Add new `UserCommand` variants and worker handling for iteration 30 operations.
4. Add state mutation and output tests before replacing any legacy command paths.
5. Run `cargo test --workspace` after implementation.

Rollback is straightforward because this change is internal to command registration and TUI/worker command handling; reverting the registry and new commands restores the previous static command table behavior.

## Open Questions

- Should `/branch` persist the new branch as a session immediately, or only fork current in-memory history until the next normal save point?
- Should `/logout` remove only rust-claude config credentials, or also offer guidance for environment variables and Claude settings that cannot be safely edited automatically?
