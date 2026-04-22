## Why

当前 Rust 版本已经完成迭代 18 的 hooks 系统，但还无法接入外部 MCP（Model Context Protocol）服务器，因此工具生态仍局限于内置工具。MCP 是 Claude Code 接入文件系统、GitHub、浏览器、设计平台等外部能力的核心扩展机制，也是后续 Agent、Skills、生态兼容能力的重要基础设施，因此需要尽快补齐。

## What Changes

- 新增 MCP 配置模型，从 `settings.json` 的 `mcpServers` 字段加载服务器配置
- 新增 MCP 客户端运行时，支持通过 stdio 启动和管理外部 MCP 服务器
- 新增 JSON-RPC over stdio 通信层，完成 `initialize`、`tools/list`、`tools/call` 基础协议
- 新增 MCP 工具代理，将远端 MCP 工具包装为本地 `Tool` trait 实现并注册到 `ToolRegistry`
- 将 MCP 工具接入 system prompt、权限系统和 QueryLoop 执行路径
- 新增 `/mcp` slash command，用于查看已连接服务器与可用工具
- 增加服务器启动失败、协议错误、调用超时的错误处理和状态展示

## Capabilities

### New Capabilities
- `mcp-config`: MCP 服务器配置建模、settings.json 加载与 user/project 配置合并
- `mcp-client-stdio`: 基于 stdio 的 MCP 客户端连接、初始化、工具列表获取与工具调用
- `mcp-tool-integration`: MCP 工具代理、本地注册、权限检查、system prompt 注入与 `/mcp` 展示

### Modified Capabilities
<!-- No existing capabilities are modified at spec level -->

## Impact

- **core crate**: 新增 MCP 配置类型、服务器与工具元数据类型
- **新增 mcp crate 或 tools 子模块**: 实现 JSON-RPC over stdio、客户端生命周期管理、协议请求/响应
- **tools crate**: 新增 MCP 代理工具，接入 `ToolRegistry`
- **cli crate**: 启动阶段加载并初始化 MCP 服务器，构建 system prompt，增加 `/mcp` 命令
- **tui crate**: 展示 MCP 服务器连接状态和命令输出（至少支持 `/mcp` 文本展示）
- **settings.json**: 新增 `mcpServers` 字段，首期仅支持 `type: "stdio"`
- **依赖**: 可能新增 JSON-RPC 相关辅助依赖，但优先复用现有 `tokio`、`serde_json` 与异步 IO 基础设施
