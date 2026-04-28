## 1. Hook Model and Parsing

- [x] 1.1 Add `SessionStart` and `SessionEnd` variants to the hook event model with string serialization/deserialization coverage.
- [x] 1.2 Add optional `once` parsing to hook configuration with default-false behavior.
- [x] 1.3 Extend PreToolUse hook response parsing to accept an optional object-valued `updatedInput` and ignore invalid shapes with a warning.
- [x] 1.4 Add unit tests for lifecycle event parsing, `once`, and `updatedInput` response parsing.

## 2. Hook Execution and QueryLoop Integration

- [x] 2.1 Extend hook input context generation for `SessionStart` and `SessionEnd` events.
- [x] 2.2 Track per-session `once` hook execution state in `HookRunner` and skip repeated matching hooks.
- [x] 2.3 Propagate approved `updatedInput` through sequential PreToolUse hook execution.
- [x] 2.4 Apply final approved `updatedInput` in QueryLoop before tool execution and pass the updated input to PostToolUse hooks.
- [x] 2.5 Wire `SessionStart` and `SessionEnd` hook invocation into non-interactive CLI and TUI session paths.
- [x] 2.6 Add tests for once execution, sequential input mutation, blocking after mutation, and session lifecycle hook invocation.
- [x] 2.7 Propagate real session IDs through session lifecycle, prompt, stop, PreToolUse, and PostToolUse hook payloads, including resumed sessions.
- [x] 2.8 Invoke `SessionEnd` hooks on both successful and error exits for print-mode and TUI session paths.

## 3. Custom Agent Registry

- [x] 3.1 Add custom agent definition types in `core` for name, description, system prompt, optional tools, optional model, and validation errors.
- [x] 3.2 Implement discovery of Markdown custom agent files under project `.claude/agents/`.
- [x] 3.3 Implement YAML front matter parsing and Markdown body extraction for custom agent definitions.
- [x] 3.4 Enforce kebab-case unique names while preserving deterministic behavior when invalid or duplicate files are encountered.
- [x] 3.5 Add registry list and lookup APIs plus unit tests for valid, missing, invalid, and duplicate agent definitions.

## 4. AgentTool Custom Agent Dispatch

- [x] 4.1 Extend AgentTool input schema with optional `agent` while preserving existing ad hoc sub-agent behavior.
- [x] 4.2 Pass the custom agent registry through AgentContext from the CLI layer.
- [x] 4.3 Resolve named custom agents in AgentTool and return a tool error when the requested agent is missing.
- [x] 4.4 Apply custom agent system prompt and optional model override to spawned sub-agent execution.
- [x] 4.5 Restrict spawned sub-agent tools using the custom agent allowlist, intersected with explicit `allowed_tools` when provided.
- [x] 4.6 Add tests for named agent resolution, missing-agent errors, model/system prompt override, and tool allowlist intersection.
- [x] 4.7 Preserve nested AgentTool delegation so sub-agents can spawn deeper sub-agents until the configured depth limit is reached.

## 5. Slash Command Integration

- [x] 5.1 Register `/agents` in the built-in slash command registry and help output.
- [x] 5.2 Implement `/agents` command output for loaded custom agents and the no-agents empty state.
- [x] 5.3 Add command validation and rendering tests for `/agents`.

## 6. Verification

- [x] 6.1 Run `cargo fmt`.
- [x] 6.2 Run `cargo test --workspace` and fix any regressions.
- [x] 6.3 Run targeted CLI/TUI tests or manual smoke checks for hook lifecycle behavior and `/agents` output if automated coverage is insufficient.
