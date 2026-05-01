## 1. Configuration Models

- [ ] 1.1 Add sandbox configuration types in `core` for enabled state, allowed paths, network policy, and sandbox-aware Bash auto-approval with safe defaults.
- [ ] 1.2 Add `PermissionMode::Auto`, parsing, serialization, display text, and CLI support for `--mode auto`.
- [ ] 1.3 Wire sandbox settings through config loading and settings merge precedence, including validation for malformed sandbox fields.
- [ ] 1.4 Add tests for Auto mode parsing and sandbox configuration precedence across defaults, settings, environment, and CLI overrides.

## 2. Sandbox Execution

- [ ] 2.1 Introduce a sandbox runner abstraction that can wrap tool process execution or return a clear unsupported-runtime error.
- [ ] 2.2 Implement macOS sandbox command construction using `sandbox-exec` with allowed filesystem paths and network policy.
- [ ] 2.3 Implement Linux sandbox command construction using `bubblewrap` or namespace-based isolation with allowed filesystem paths and network policy.
- [ ] 2.4 Integrate sandbox execution into BashTool while preserving unsandboxed behavior when sandboxing is disabled.
- [ ] 2.5 Add tests for allowed path canonicalization, unsupported sandbox runtime handling, no-network command construction, and Bash behavior inside and outside sandbox mode.

## 3. Auto Mode Safety Checks

- [ ] 3.1 Add deterministic safety-check helpers for path scope validation, dangerous Bash command detection, and suspicious output detection.
- [ ] 3.2 Extend permission evaluation so Auto mode allows low-risk read, edit, and Bash operations while preserving explicit deny precedence.
- [ ] 3.3 Make Auto mode return confirmation-required behavior for unsafe paths, dangerous Bash patterns, sandbox failures, or suspicious follow-up conditions.
- [ ] 3.4 Implement `autoAllowBashIfSandboxed` so sandboxed Bash can skip normal confirmation only when explicit denies and Auto safety checks do not block it.
- [ ] 3.5 Add unit tests for Auto approval, explicit deny precedence, dangerous command escalation, unsafe path escalation, and sandbox-aware Bash approval.

## 4. Query Loop and Runtime Integration

- [ ] 4.1 Propagate effective sandbox configuration and Auto mode into the query loop, tool context, and permission manager without changing existing modes.
- [ ] 4.2 Ensure confirmation fallback uses the existing permission result path when Auto mode safety checks require user confirmation.
- [ ] 4.3 Ensure sandbox network policy applies only to sandboxed tool executions and does not block model API or MCP transport requests.
- [ ] 4.4 Add integration-style tests or mocked query-loop tests covering Auto-approved tools, Auto confirmation fallback, and sandbox execution errors.

## 5. Verification

- [ ] 5.1 Run targeted `core` tests for permission modes, config merging, and sandbox settings validation.
- [ ] 5.2 Run targeted `tools` tests for Bash sandbox wrapping and safety-check behavior.
- [ ] 5.3 Run `cargo test --workspace` and fix regressions.
- [ ] 5.4 Manually verify representative CLI behavior for `--mode auto`, sandbox disabled defaults, sandbox enabled unsupported-runtime errors, and sandbox network policy where the host runtime supports it.
