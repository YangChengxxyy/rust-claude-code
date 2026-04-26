## Context

The current codebase already has the right foundations for this iteration:

- `AppState` stores a session-level `cwd`.
- `BashTool` accepts an optional `workdir`, but otherwise does not read or update `AppState.cwd`.
- `ToolContext` carries `app_state` and optional agent context.
- `TuiBridge` already uses oneshot channels for permission requests.
- `WebSearchTool` already has a `SearchBackend` abstraction, domain filtering, and structured formatted output.

The goal is to use these existing seams instead of introducing a separate interaction or provider framework.

## Goals / Non-Goals

**Goals:**

- Preserve Bash CWD across commands in a way that matches normal shell usage.
- Let the model ask the user a bounded structured question during an agent turn.
- Make WebSearch useful with at least one real provider.
- Keep all new capabilities testable without requiring live network or terminal interaction in unit tests.

**Non-Goals:**

- Project-root CWD confinement. Iteration 26 intentionally uses normal shell CWD semantics; later sandbox work can add stricter boundaries.
- A general-purpose form framework for arbitrary user input.
- Multiple production WebSearch providers in the same iteration. The provider abstraction should allow more later, but one real backend is sufficient.
- Streaming AskUserQuestion UI. The question arrives as a complete tool input.

## Decisions

### D1: Bash persistent CWD follows normal shell semantics

**Choice**: Bash commands start in the session CWD unless `workdir` is explicitly supplied. After the shell exits, the final shell working directory becomes the new session CWD when it can be captured and resolved. This iteration does not restrict the final CWD to the project root.

**Rationale**: This matches user expectations for `cd /tmp && pwd` followed by `ls`. Root confinement belongs with sandbox/auto-mode policy, not with the basic Bash tool behavior.

**Boundary details**:

- If `workdir` is omitted, the command starts in `AppState.cwd`.
- If `workdir` is supplied, the command starts there for this invocation.
- If the command completes and reports a valid final directory, `AppState.cwd` is updated to that directory.
- If the command times out, fails to start, or the final directory cannot be parsed/resolved, `AppState.cwd` remains unchanged.
- Nonzero shell exit status does not automatically prevent CWD update if the final directory was captured, because shells can change directory before a later command fails.

### D2: Bash should capture final CWD without changing visible command output

**Choice**: Wrap Bash execution so the implementation can capture the shell's final `pwd` separately from the user-visible stdout/stderr content.

**Alternatives considered**:

- Parse user-visible output for `pwd`. This is unreliable and changes behavior depending on whether the command prints a directory.
- Only detect simple `cd <path>` commands. This misses common shell forms like `cd dir && cargo test`, `pushd`, shell functions, or conditionals.

**Rationale**: The tool result should remain focused on the command output while the runtime still learns the final working directory.

### D3: AskUserQuestion uses a ToolContext callback rather than depending directly on TUI

**Choice**: Extend tool execution context with an optional user-question callback. `AskUserQuestionTool` validates its input and asks through that callback. The CLI supplies a callback backed by `TuiBridge` in TUI mode and a deterministic fallback in non-interactive mode.

**Alternatives considered**:

- Make the tool depend directly on `rust-claude-tui`. This would couple the tools crate to the UI crate and complicate tests.
- Implement AskUserQuestion only as a slash command. The model needs to ask from inside the tool loop, so it must be a tool.

**Rationale**: This mirrors the existing permission-dialog shape while keeping the tool layer UI-agnostic.

### D4: AskUserQuestion has bounded options and optional custom input

**Choice**: The tool input contains `question`, `options`, and `allow_custom`. Each option has `label` and `description`. The response includes the selected label and answer text, or a custom answer when custom input is allowed.

**Rationale**: Bounded choices keep the model from turning every uncertainty into an open-ended interruption, while custom input remains available when the tool explicitly allows it.

### D5: Non-interactive AskUserQuestion chooses a deterministic fallback

**Choice**: Without an interactive bridge, AskUserQuestion returns the first option when options exist. If there are no options and custom input is not available, the tool returns an error. If stdin support is later added, it can replace this fallback without changing the tool schema.

**Rationale**: `--print` and tests must not hang waiting for UI. Deterministic fallback makes behavior predictable and easy to document.

### D6: WebSearch provider selection is configuration-driven, with env secrets

**Choice**: Select the WebSearch backend from settings/configuration or environment, and read provider credentials from environment variables or settings fields. The first real provider should be chosen for simple API shape and testability. Brave Search is preferred if no stronger local constraint appears during implementation; Tavily or SearXNG remain valid alternatives if they fit the existing HTTP stack better.

**Rationale**: `WebSearchTool` already has the backend abstraction. The important iteration-26 upgrade is replacing the dummy provider with a real configured backend while keeping provider details out of the model-facing tool schema.

## Risks / Trade-offs

- **[Bash wrapper can leak marker text]** -> Use a hard-to-collide marker and strip only the final marker line from captured output. Tests should cover user output that contains similar text.
- **[Concurrent Bash calls could race on CWD]** -> Bash is already non-concurrency-safe, so serial execution protects session CWD updates.
- **[AskUserQuestion can over-interrupt users]** -> System prompt/tool description should frame it as a clarification tool for meaningful choices, not routine confirmation.
- **[Clipboard-style modal complexity repeats]** -> Keep modal state small and reuse permission dialog patterns where practical.
- **[Search providers require network/API keys]** -> Unit tests should use fake HTTP/backends; live provider tests should be ignored or environment-gated.
- **[Provider response formats differ]** -> Normalize into the existing `SearchResult { title, url, summary }` type at the backend boundary.

