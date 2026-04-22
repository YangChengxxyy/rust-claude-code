## 1. MCP 配置模型与 settings 集成

- [x] 1.1 在 `core` crate 中新增 MCP 配置类型（`McpServerConfig`、`McpTransportType`、stdio 配置结构、运行时状态类型）
- [x] 1.2 为 `ClaudeSettings` 增加 `mcp_servers` 字段并映射 `mcpServers` JSON key
- [x] 1.3 实现 user/project settings 的 `mcpServers` 合并规则（按 server name 合并，同名由高优先级覆盖）
- [x] 1.4 为 MCP 配置加载、transport 过滤和合并逻辑补充单元测试
- [x] 1.5 验证 `cargo test -p rust-claude-core` 通过

## 2. MCP crate 与 stdio JSON-RPC 基础设施

- [x] 2.1 新增 `crates/mcp` 并加入 workspace 依赖图
- [x] 2.2 实现 JSON-RPC 2.0 消息类型与 `Content-Length` framing 读写器
- [x] 2.3 实现 stdio 传输层，负责启动子进程、持有 stdin/stdout、发送请求并接收响应
- [x] 2.4 实现 `initialize`、`tools/list`、`tools/call` 三个 MCP 基础请求封装
- [x] 2.5 实现超时、进程启动失败、协议错误和 malformed JSON 的错误类型与处理逻辑
- [x] 2.6 为 framing、request/response 匹配、超时和协议错误补充单元测试
- [x] 2.7 验证 `cargo test -p rust-claude-mcp` 通过

## 3. MCP manager 与服务器生命周期管理

- [x] 3.1 在 `mcp` crate 中实现 `McpManager`，按配置启动多个 stdio servers
- [x] 3.2 为每个 server 执行 `initialize` 和 `tools/list`，记录 connected/failed 状态与 discovered tools
- [x] 3.3 暴露查询接口，用于按 server/tool 名称查找工具定义和发起 `tools/call`
- [x] 3.4 实现"单个 server 失败不阻断整体启动"的容错行为
- [x] 3.5 为 manager 启动流程、失败隔离和工具快照补充单元测试
- [x] 3.6 验证 `cargo test -p rust-claude-mcp` 通过

## 4. MCP 工具代理与 ToolRegistry 集成

- [x] 4.1 在 `tools` crate 中新增 MCP 代理工具实现，工具名格式为 `mcp__<server>__<tool>`
- [x] 4.2 将远端 description 和 input schema 映射到本地 `ToolInfo`
- [x] 4.3 在 MCP 代理工具的 `execute()` 中调用 `McpManager` 的 `tools/call`，将结果转换为本地 `ToolResult`
- [x] 4.4 定义首期安全策略：MCP 代理工具默认非只读、非并发安全
- [x] 4.5 为代理工具命名、schema 转发、错误转换补充单元测试
- [x] 4.6 验证 `cargo test -p rust-claude-tools` 通过

## 5. CLI 启动接线与 system prompt 集成

- [x] 5.1 在 `main.rs` 中加载合并后的 `mcpServers` 配置并初始化 `McpManager`
- [x] 5.2 在工具注册阶段把 discovered MCP tools 动态注册进 `ToolRegistry`
- [x] 5.3 确保 `--allowed-tools` / `--disallowed-tools` 对 MCP 工具同样生效
- [x] 5.4 在 system prompt 构建中注入 MCP 工具描述，使模型可见外部工具能力
- [x] 5.5 为无 MCP、部分 server 失败、工具过滤等场景补充 CLI 层测试
- [x] 5.6 验证 `cargo test -p rust-claude-cli` 通过

## 6. Slash command 与 TUI/文本展示

- [x] 6.1 新增 `/mcp` slash command，输出 server 名称、连接状态、失败摘要和工具列表
- [x] 6.2 在 TUI 模式下复用现有消息输出路径展示 `/mcp` 结果
- [x] 6.3 确保 print/非交互模式也能正确展示 MCP 状态信息
- [x] 6.4 为 `/mcp` 的无配置、连接成功、连接失败场景补充测试
- [x] 6.5 验证 `cargo test -p rust-claude-tui` 与 `cargo test -p rust-claude-cli` 通过

## 7. 集成验证与文档更新

- [x] 7.1 使用一个真实 stdio MCP server（如 filesystem 类 server）完成端到端 smoke test
- [x] 7.2 验证 MCP 工具经过现有权限系统，必要时会触发确认/拒绝逻辑
- [x] 7.3 验证 MCP 工具可被 QueryLoop 调用并返回正确结果
- [x] 7.4 运行 `cargo test --workspace` 和 `cargo build` 确认全仓通过
- [x] 7.5 更新 `doc/requirement.md` 中迭代 19 的状态与完成记录
