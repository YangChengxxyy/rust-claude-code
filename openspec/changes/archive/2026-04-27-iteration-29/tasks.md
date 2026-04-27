## 1. Auto Memory Core

- [x] 1.1 Add core types and helpers for automatic memory candidates, automatic-memory disable detection, and dedup-aware save outcomes
- [x] 1.2 Refactor manual `/memory remember` persistence to call the shared dedup-aware memory save helper without changing existing manual command behavior
- [x] 1.3 Update memory index rebuild and error reporting paths so automatic writes can report saved, updated, skipped, disabled, and failed outcomes
- [x] 1.4 Add core memory tests for auto-memory disable flags, duplicate path updates, duplicate topic-name updates, new writes, and manual writes while auto-memory is disabled

## 2. Prompt And QueryLoop Integration

- [x] 2.1 Update the memory contract prompt to describe automatic save conditions, no-save rules, sensitive-content avoidance, and disabled behavior
- [x] 2.2 Add an automatic memory write path in the agent loop that accepts model-requested memory candidates and persists them through the shared helper
- [x] 2.3 Ensure automatic memory failures do not fail the user-facing assistant turn and are surfaced as debug/tool/system outcomes
- [x] 2.4 Add QueryLoop or prompt-construction tests covering enabled auto-memory guidance, disabled guidance, and automatic write no-op behavior

## 3. Doctor Command

- [x] 3.1 Add `/doctor` to the TUI slash command registry, suggestions, help output, command validation, and `UserCommand` dispatch
- [x] 3.2 Implement a diagnostic report builder for API readiness, effective configuration sources, config parse errors, MCP configuration, local `git`/`gh` availability, and permission file integrity
- [x] 3.3 Wire `/doctor` handling in the TUI background worker and render the report as a system or assistant-style message without requiring an API call
- [x] 3.4 Add tests for `/doctor` registration, dispatch, valid/missing credential reporting, malformed config reporting, missing optional `gh`, and invalid permission rules

## 4. Review Command

- [x] 4.1 Add `/review [pr-number-or-url]` to the TUI slash command registry, suggestions, help output, command validation, and `UserCommand` dispatch
- [x] 4.2 Implement review diff collection for current branch, including repository detection, base branch fallback, no-diff reporting, and deterministic truncation notices
- [x] 4.3 Implement PR number/URL diff collection using `gh` when available, with clear fallback or error messages when `gh` is missing
- [x] 4.4 Build the structured review prompt and submit it through the existing QueryLoop path so output prioritizes findings, severity, file/line references, no-finding statements, and residual risk notes
- [x] 4.5 Add tests for current-branch review dispatch, no Git repository, no diff, missing `gh` for PR input, diff truncation, and review prompt structure

## 5. Integration Verification

- [x] 5.1 Run `cargo test -p rust-claude-core` and fix memory-related failures
- [x] 5.2 Run `cargo test -p rust-claude-cli` and fix QueryLoop or prompt integration failures
- [x] 5.3 Run `cargo test -p rust-claude-tui` and fix slash command, doctor, and review failures
- [x] 5.4 Run `cargo test --workspace` and confirm the full workspace passes
- [x] 5.5 Manually smoke-test TUI `/doctor`, `/review` without arguments, and `/review` with a PR-like argument in a repository where `gh` is unavailable or available
