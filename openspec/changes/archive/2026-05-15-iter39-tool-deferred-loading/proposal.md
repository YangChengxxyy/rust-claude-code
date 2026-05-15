## Why

当前所有注册工具的完整 JSON Schema 都在每次 API 请求中发送。随着 MCP 工具和内置工具数量增加，工具定义占用大量 prompt 空间，压缩了模型实际可用的上下文窗口。原版通过 `shouldDefer` 标记低频工具，仅发送名称，模型按需通过搜索加载完整 schema。本迭代实现工具延迟加载机制，显著减少初始请求的 prompt 空间占用。

## What Changes

- `Tool` trait 新增 `should_defer(&self) -> bool` 方法（默认 `false`）
- 标记低频/大型工具为延迟加载：所有 MCP 代理工具、`WebSearchTool`、`WebFetchTool`、`LspTool`、`TaskTool`（get/list/update/stop）、`ExitPlanModeTool`、`NotebookEditTool`、`MonitorTool`
- 新增 `ToolSearchTool`：模型可通过关键字搜索或精确选择发现延迟工具，返回完整 JSON Schema
- `ToolRegistry` 新增延迟工具管理：`get_deferred_tools()`、`get_non_deferred_tools()`、`search_tools()`、`estimate_deferred_schema_tokens()`
- `ApiTool` 新增 `deferred: bool` 字段，构建请求时延迟工具仅发送 `{ name, deferred: true }`
- `QueryLoop` 追踪 `discovered_tools: HashSet<String>`，已发现的延迟工具自动升级为非延迟
- 自动阈值：仅当延迟工具 schema 总 token 估算超过上下文窗口 10% 时才启用延迟机制

## Capabilities

### New Capabilities
- `tool-deferred-loading`: 工具延迟加载机制。标记工具为延迟、发送精简定义、自动发现升级、阈值控制。
- `tool-search`: 模型侧工具发现搜索。`ToolSearchTool` 实现，支持关键字搜索、`select:ToolName` 精确选择、评分排序。

### Modified Capabilities
- (无 spec 级别需求变更。现有工具标记 deferred 为实现细节，不改变其功能行为。)

## Impact

- `tools` crate: `tool.rs` (trait 扩展)、`tool_search.rs` (新文件)、`registry.rs` (新方法)、各工具实现 (`should_defer` 覆盖)
- `api` crate: `types.rs` (`ApiTool` 新增字段)、请求构建逻辑
- `sdk` crate: `agent_loop.rs` (discovered_tools 追踪、每轮工具列表动态组装)
- `core` crate: (无直接修改，Config 不涉及)
- 无新增外部依赖

