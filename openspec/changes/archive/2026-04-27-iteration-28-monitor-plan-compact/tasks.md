## 1. Monitor tool implementation

- [x] 1.1 Add a `MonitorTool` module in `crates/tools` with input fields `command`, `pattern`, `timeout`, and an optional capture limit; expose tool metadata through the existing `Tool` trait
- [x] 1.2 Implement async process spawning for monitor commands with concurrent stdout/stderr line reading, regex matching, omitted-line counting, matched-line truncation, and exit-status reporting
- [x] 1.3 Enforce timeout cleanup by terminating the child process when the timeout expires and returning a timeout result with matched output collected so far
- [x] 1.4 Wire monitor command permission checks through the existing permission path before process spawn so denied commands never start
- [x] 1.5 Register `MonitorTool` in the default tool registry and ensure allow/deny filtering can include or exclude it by name
- [x] 1.6 Add unit tests for matched output, omitted output count, timeout cleanup, capture-limit truncation, and denied-command behavior

## 2. Plan mode tools

- [x] 2.1 Add shared state support for saving and clearing the previous permission mode during an agent-driven plan-mode transition
- [x] 2.2 Implement `EnterPlanModeTool` to save the current permission mode, set mode to `Plan`, and return an idempotent result when already in plan mode
- [x] 2.3 Implement `ExitPlanModeTool` to require a non-empty plan summary, restore the saved permission mode when present, and return a clear no-active-transition result otherwise
- [x] 2.4 Register both plan-mode tools in the default tool registry with appropriate read-only/concurrency metadata
- [x] 2.5 Add tests covering enter, enter-while-already-plan, exit-with-summary, exit-without-saved-mode, and denial of a mutating tool after entering plan mode

## 3. Compaction context enrichment

- [x] 3.1 Add bounded compaction context fields for project guidance, used MCP tools, and recent permission decisions without changing the existing summary-plus-preserved-messages replacement model
- [x] 3.2 Collect or derive project `CLAUDE.md` guidance for compaction and omit the section gracefully when no guidance is available
- [x] 3.3 Track session-local MCP tool usage sufficiently to include bounded MCP server/tool names in compaction context when available
- [x] 3.4 Track recent permission decisions with a small configurable bound and include only the most recent decisions during compaction
- [x] 3.5 Update the compaction prompt/context assembly so enriched sections are included in the LLM summarization input and bounded to avoid excessive prompt growth
- [x] 3.6 Add tests for compaction with project guidance, without project guidance, with MCP usage, without MCP usage, and with permission decisions exceeding the bound

## 4. `/compact` strategy arguments

- [x] 4.1 Define named compact retention strategies for `default`, `aggressive`, and `preserve-recent`, mapping each to concrete compaction configuration values
- [x] 4.2 Update TUI slash command parsing so `/compact` accepts an optional strategy argument and rejects unknown strategy names before starting compaction
- [x] 4.3 Update `/help` output to document `/compact [default|aggressive|preserve-recent]` or equivalent supported strategy syntax
- [x] 4.4 Route the selected compaction strategy through the worker/query-loop path to the compaction service
- [x] 4.5 Add tests for no-argument default behavior, named strategy behavior, unknown strategy error, and help output

## 5. Integration and verification

- [x] 5.1 Run `cargo test -p rust-claude-tools` and fix regressions in tool-level tests
- [x] 5.2 Run `cargo test -p rust-claude-core` and fix regressions in state or compaction tests
- [x] 5.3 Run `cargo test -p rust-claude-cli` and fix regressions in query-loop, compaction, and permission integration tests
- [x] 5.4 Run `cargo test -p rust-claude-tui` and fix regressions in slash command and status feedback tests
- [x] 5.5 Run `cargo test --workspace` to verify the full workspace
- [x] 5.6 Run `cargo build` to verify all crates compile cleanly
