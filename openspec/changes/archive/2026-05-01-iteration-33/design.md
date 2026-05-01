## Context

The current permission system controls whether a tool call is allowed, denied, or requires confirmation, but allowed commands still run directly on the host process with normal filesystem and network access. This is acceptable for trusted local work, but it is too broad for unfamiliar repositories, automated workflows, or users who want an Auto mode that avoids frequent prompts without giving up safety checks.

Sandboxing and Auto mode cut across configuration, permission evaluation, query-loop execution, and tool process spawning. The implementation must preserve existing default behavior unless sandboxing or Auto mode is explicitly enabled.

## Goals / Non-Goals

**Goals:**

- Add a sandbox configuration model with enabled state, allowed paths, network policy, and sandbox-aware Bash approval.
- Execute Bash commands inside a platform sandbox when sandboxing is enabled.
- Add `PermissionMode::Auto` and `--mode auto` so low-risk tool calls can proceed without prompting.
- Run deterministic safety checks before and after Auto-approved tool calls.
- Fall back to the existing confirmation path when Auto safety checks fail.
- Keep default, accept-edits, bypass, plan, and dont-ask behavior compatible.

**Non-Goals:**

- Perfect containment against kernel or sandbox runtime vulnerabilities.
- Container orchestration, VM isolation, or remote execution.
- Full policy language for sandbox profiles beyond allowed paths and network on/off.
- Sandbox enforcement for API requests or MCP remote transport connections.
- TUI redesign for permission prompts beyond using the existing confirmation mechanism.

## Decisions

### Decision: Represent sandbox settings in `core` and enforce them at tool execution boundaries

Sandbox configuration belongs in `core` because it participates in config merging and permission decisions. Enforcement belongs closest to the operation that launches host processes or touches files, starting with Bash and path-aware file tools.

Alternatives considered:

- Put sandbox state only in `tools`: rejected because CLI parsing, config precedence, and permission evaluation also need to see it.
- Wrap the whole CLI process in a sandbox: rejected because the CLI must still access config, sessions, API credentials, and network while individual tool execution can be constrained.

### Decision: Use platform adapters behind a small sandbox runner abstraction

The tool layer should request sandboxed execution through an abstraction that can build macOS `sandbox-exec` invocations, Linux `bubblewrap` invocations, or return a clear unsupported-platform error. The initial implementation should prefer explicit command wrapping over large runtime refactors.

Alternatives considered:

- Implement Linux namespaces directly: rejected for the first iteration because `bubblewrap` is safer and smaller to integrate.
- Only support macOS first: rejected because the requirement explicitly covers Linux behavior.

### Decision: Treat network policy as part of sandbox execution, not global networking

`sandbox.network` controls whether sandboxed tool calls may use outbound network. It does not block the model API client, provider routing, session persistence, or MCP clients running outside the sandbox.

Alternatives considered:

- Apply process-wide network blocking: rejected because it would break model traffic and remote MCP use.

### Decision: Auto mode is a permission mode with safety gates, not bypass mode

`PermissionMode::Auto` should automatically allow operations only after safety checks pass. If a tool is denied by explicit rules, it remains denied. If checks identify risk, the result should be `NeedsConfirmation` so existing confirmation flows can handle it.

Alternatives considered:

- Make Auto equivalent to bypass inside sandbox: rejected because sandbox availability varies and file path checks still matter.
- Add per-tool ad hoc auto flags: rejected because mode-level semantics are clearer and easier to test.

### Decision: Start with deterministic safety checks

Safety checks should cover path scope, dangerous command patterns, and suspicious command output using deterministic rules. This keeps behavior testable and avoids introducing external classifiers.

Alternatives considered:

- Use model-based safety analysis: rejected because it adds cost, latency, nondeterminism, and bootstrapping concerns.

## Risks / Trade-offs

- [Risk] `sandbox-exec` is deprecated on recent macOS versions and may not be available in all environments. → Mitigation: detect availability and return a clear unsupported error rather than silently running unsandboxed.
- [Risk] `bubblewrap` may not be installed or permitted by host policy. → Mitigation: detect at startup or first sandboxed execution and surface actionable errors.
- [Risk] Path normalization bugs could allow access outside configured roots. → Mitigation: canonicalize paths before comparison and add unit tests for relative paths, symlinks, home paths, and project-root paths.
- [Risk] Dangerous command detection may produce false positives. → Mitigation: fall back to confirmation instead of denial when Auto checks are uncertain.
- [Risk] Network blocking differs by platform adapter. → Mitigation: document adapter behavior and cover no-network command construction in tests.

## Migration Plan

- Add sandbox and Auto-mode types with defaults that preserve current behavior.
- Add CLI/config parsing behind optional flags and fields.
- Add sandbox runner adapters and tests for command construction.
- Wire Bash execution through the sandbox runner only when enabled.
- Extend permission evaluation for Auto mode and safety-check fallback.
- Add verification tests before documenting the feature as usable.

Rollback is straightforward: disable `sandbox.enabled`, avoid `--mode auto`, or revert to existing permission modes. Existing config files without sandbox fields continue to parse with default sandbox disabled.

## Open Questions

- Should sandbox allowed paths default to only the project root, or include config/session/cache directories needed by tools? The proposed default is project root for tools, with CLI/runtime files accessed outside sandbox.
- Should Auto mode inspect file write contents for secrets or only path and command metadata in this iteration? The proposed first iteration limits checks to path scope, command risk, and output anomalies.
