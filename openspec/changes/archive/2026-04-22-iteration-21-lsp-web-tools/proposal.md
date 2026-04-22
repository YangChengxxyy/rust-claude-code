## Why

当前 Rust Claude Code 已经具备本地文件、Bash、Glob/Grep、MCP、Agent/Task 等能力，但仍缺少两类高频工具能力：代码级语义导航和受控的 Web 信息获取。没有 LSP，模型只能依赖文本搜索完成代码理解；没有 WebFetch/WebSearch，模型无法在需要外部资料时安全、结构化地获取网页内容与搜索结果。

## What Changes

- 新增 `LspTool`，支持 `goToDefinition`、`findReferences`、`hover`、`documentSymbol`、`workspaceSymbol` 等代码导航操作
- 新增 LSP 服务器生命周期管理与 JSON-RPC over stdio 通信层，首期支持 Rust、TypeScript、Python 的常见 LSP 服务端
- 新增 `WebFetchTool`，支持抓取网页、提取内容并转为 Markdown 文本，附带基础缓存与内容截断
- 新增 `WebSearchTool`，支持调用可配置搜索后端并返回结构化搜索结果
- 将上述新工具接入现有 ToolRegistry、权限系统、system prompt 和 QueryLoop 执行路径

## Capabilities

### New Capabilities
- `lsp-tool`: 通过语言服务器协议提供代码级语义导航、符号查询和悬停信息
- `web-fetch`: 获取网页内容、转换为 Markdown、按提示做提取或总结，并支持缓存与截断
- `web-search`: 调用搜索后端返回结构化搜索结果，并支持允许/阻止域名过滤

### Modified Capabilities
<!-- No existing spec-level requirements are changing. -->

## Impact

- **tools crate**: 新增 `LspTool`、`WebFetchTool`、`WebSearchTool` 及其辅助模块
- **可能新增共享基础设施模块或 crate**: 用于 LSP 的 JSON-RPC/stdio 传输、HTTP 获取、缓存、搜索后端抽象
- **cli crate**: 启动时可能需要根据项目语言环境初始化/按需启动 LSP 服务；system prompt 需要暴露新工具描述
- **权限系统**: 新工具需沿用现有权限模型；Web 工具默认按非只读工具处理，除非实现明确只读策略
- **测试**: 增加 LSP 集成测试、Web 获取与搜索工具测试；工作区测试需继续保持通过
