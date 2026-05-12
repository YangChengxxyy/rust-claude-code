## 1. Tool Trait and Registry Extensions

- [x] 1.1 Add `should_defer(&self) -> bool` to `Tool` trait with default `false` in `crates/tools/src/tool.rs`
- [x] 1.2 Add `should_defer: bool` field to `RegisteredTool` and capture it in `ToolRegistry::register()`
- [x] 1.3 Implement `get_deferred_tools()` and `get_non_deferred_tools()` on `ToolRegistry`
- [x] 1.4 Implement `search_tools(query: &str, max: usize)` on `ToolRegistry` (name + description keyword search)
- [x] 1.5 Implement `estimate_deferred_schema_tokens()` on `ToolRegistry` (schema JSON string length / 4 heuristic)
- [x] 1.6 Add unit tests for registry partition and token estimation in `crates/tools/src/registry.rs`

## 2. ApiTool Deferred Serialization

- [x] 2.1 Add `deferred: Option<bool>` field to `ApiTool` in `crates/api/src/types.rs`
- [x] 2.2 Update `ApiTool::new()` to set `deferred: None`
- [x] 2.3 Add `ApiTool::deferred(name)` constructor for minimal deferred representation
- [x] 2.4 Add serialization tests verifying deferred tool omits `description` and `input_schema`
- [x] 2.5 Add deserialization test for deferred tool JSON from API

## 3. Mark Target Tools as Deferred

- [x] 3.1 Override `should_defer() -> true` for all `McpProxyTool` instances in `crates/tools/src/mcp/`
- [x] 3.2 Override `should_defer() -> true` for `WebSearchTool` and `WebFetchTool`
- [x] 3.3 Override `should_defer() -> true` for `LspTool`
- [x] 3.4 Override `should_defer() -> true` for `TaskTool` (get/list/update/stop subcommands)
- [x] 3.5 Override `should_defer() -> true` for `ExitPlanModeTool`
- [x] 3.6 Override `should_defer() -> true` for `NotebookEditTool` and `MonitorTool`
- [x] 3.7 Verify core tools (`Bash`, `FileRead`, `FileEdit`, `FileWrite`, `Grep`, `Glob`, `TodoWrite`, `EnterPlanMode`) remain non-deferred

## 4. ToolSearchTool Implementation

- [x] 4.1 Create `crates/tools/src/tool_search.rs` with `ToolSearchTool` struct implementing `Tool`
- [x] 4.2 Implement input schema parsing (`query`, `max_results` with default 5 and max 20)
- [x] 4.3 Implement exact selection logic (`select:ToolName` prefix match on deferred tools)
- [x] 4.4 Implement keyword search with CamelCase and `__` tokenization on tool names and descriptions
- [x] 4.5 Implement scoring: name match weight 2x, description match weight 1x; sort by score desc, then name asc
- [x] 4.6 Implement output format: JSON array of `{ name, description, input_schema }` objects
- [x] 4.7 Ensure `ToolSearchTool::should_defer()` returns `false`
- [x] 4.8 Add unit tests for exact selection, keyword search, CamelCase splitting, MCP name splitting, and scoring

## 5. QueryLoop Integration

- [x] 5.1 Add `discovered_tools: HashSet<String>` field to `QueryLoop` in `crates/cli/src/query_loop.rs`
- [x] 5.2 Add `get_context_window_size(model: &str) -> usize` helper (map model names to 200K default)
- [x] 5.3 Modify tool list assembly logic: collect non-deferred tools + discovered deferred tools with full schema + undiscovered deferred tools with minimal definition
- [x] 5.4 Add threshold check before assembling: only use deferred representation if `estimate_deferred_schema_tokens() > context_window * 0.10`
- [x] 5.5 After tool execution, add any executed deferred tool names to `discovered_tools`
- [x] 5.6 Ensure `ToolSearchTool` result consumption does NOT automatically add to `discovered_tools` (only actual invocation does)

## 6. Integration and Verification

- [x] 6.1 Register `ToolSearchTool` in CLI tool initialization alongside other tools
- [x] 6.2 Run `cargo check --workspace` and fix any compilation errors
- [x] 6.3 Run `cargo test --workspace` and ensure all existing tests pass
- [x] 6.4 Add integration test: mock registry with 20 deferred tools, verify `QueryLoop` sends minimal definitions when threshold exceeded
- [x] 6.5 Add integration test: simulate model invoking deferred tool, verify next request includes full schema
- [x] 6.6 Update AGENTS.md or relevant docs if CLI behavior changes are user-visible
