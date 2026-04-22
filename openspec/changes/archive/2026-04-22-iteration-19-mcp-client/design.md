## Context

当前 Rust Claude Code 已完成 hooks 系统、配置合并、QueryLoop、ToolRegistry、system prompt 注入和 TUI slash commands，但工具来源仍然只限于内置 Rust 实现。原版 Claude Code 的 MCP 能力允许通过外部服务器动态扩展工具集，这是连接 GitHub、浏览器、文件系统和第三方平台的关键基础设施。

本仓库当前与 MCP 最相关的现状如下：
- `ClaudeSettings` 已支持从 user/project `settings.json` 读取并合并结构化配置，适合继续承载 `mcpServers`
- `ToolRegistry` 目前只注册静态内置工具，尚不支持运行时动态加载外部工具
- `QueryLoop` 已具备完整工具执行、权限检查、hooks 和 system prompt 集成路径，可复用到 MCP 工具
- `main.rs` 已集中完成配置解析、工具注册、QueryLoop 构建与 slash command 接线，适合作为 MCP 启动入口

原版 TS MCP 能力覆盖多种 transport（stdio/SSE/HTTP/WebSocket）、OAuth、资源和 prompts，但本迭代目标是先打通最小闭环：**stdio MCP server → tools/list → tools/call → ToolRegistry → QueryLoop**。

## Goals / Non-Goals

**Goals:**
- 在 `settings.json` 中定义并加载 `mcpServers` 配置
- 支持通过 stdio 启动 MCP 服务器并完成 `initialize`
- 支持 `tools/list` 获取远端工具定义，并包装成本地 `Tool` trait
- 支持 `tools/call` 调用远端工具，并将结果转为本地 `ToolResult`
- 将 MCP 工具接入 `ToolRegistry`、权限系统、system prompt 和 `/mcp` slash command
- 为后续扩展到更多 transport、resources、prompts 留出清晰边界

**Non-Goals:**
- 不实现 SSE、HTTP、WebSocket transport
- 不实现 OAuth、headers、远程认证与重连策略
- 不实现 MCP `resources/list`、`prompts/list`、sampling、elicitation 等高级协议能力
- 不实现独立 `mcp` crate 之外的完整生态命令面板或状态栏可视化
- 不实现服务器热重载、自动重连和后台健康检查

## Decisions

### D1: 首期只支持 stdio transport

**选择**: `mcpServers` 首期仅接受 `type: "stdio"` 配置，字段包括 `command`、`args`、`env`、`cwd`。

**理由**:
- 与 requirement.md 的迭代 19 最小目标一致，最快建立可用链路
- 现有 Rust 代码已经具备成熟的 `tokio::process::Command` 使用经验（BashTool、HookRunner）
- 避免在第一版中引入远程 transport、认证、网络重连等大量横向复杂度

**备选方案**:
- 直接设计统一多 transport 抽象：会显著推高首轮复杂度，且无法在短周期内稳定验证
- 先做内存 mock server：不能验证真实 MCP server 集成价值

### D2: 新增独立 `mcp` crate，而不是塞进 `tools` crate

**选择**: 在 workspace 中新增 `crates/mcp`，负责 JSON-RPC over stdio、进程生命周期、协议请求/响应和服务器会话管理；`tools` crate 只保留 MCP 代理工具适配层。

**理由**:
- MCP 既不是普通工具，也不是 CLI 逻辑，而是独立基础设施层
- 能避免把协议细节塞进 `tools` crate，保持 `Tool` trait 实现与传输层解耦
- 为后续扩展更多 transport 或 resources/prompts 保留干净模块边界

**备选方案**:
- 放在 `tools` crate：会让协议层与工具抽象耦合过深
- 放在 `cli` crate：不利于单测和未来多入口复用

### D3: 使用最小 JSON-RPC 2.0 over stdio 实现

**选择**: MCP 客户端使用 `Content-Length` framing 的 stdio JSON-RPC 读写循环，首期实现：
- `initialize`
- `tools/list`
- `tools/call`

**理由**:
- 这是 MCP server 最基础、最通用的交互路径
- 可以不依赖重量级 SDK，保持 Rust 版本的可控性和可测试性
- 便于对请求超时、协议错误、进程退出进行精细处理

**备选方案**:
- 直接引入第三方 MCP Rust SDK：生态尚不稳定，且会把协议边界与错误类型交给外部库

### D4: MCP 工具在本地注册为动态代理工具

**选择**: 每个远端工具在本地包装为一个 `Tool` trait 实现，命名格式为 `mcp__<server_name>__<tool_name>`，并在 `ToolRegistry` 中像内置工具一样注册。

**理由**:
- 最大程度复用现有 QueryLoop、权限系统、system prompt 注入和工具过滤逻辑
- 权限规则可以直接基于最终工具名工作，无需为 MCP 特判一整套权限分支
- 符合原版 Claude Code 的工具命名习惯，便于后续迁移

**备选方案**:
- 单一 `McpTool` + 参数中再区分 server/tool：会削弱模型对具体工具能力的可见性，也不利于权限粒度控制

### D5: MCP server 生命周期由 `main.rs` 启动期统一管理

**选择**: 程序启动时根据合并后的 `mcpServers` 配置创建 `McpManager`，完成服务器连接、工具发现和代理工具注册；QueryLoop 运行期间复用已建立的 manager。

**理由**:
- 与现有工具注册流程一致，避免在查询过程中临时建连带来的延迟和失败复杂度
- `/mcp` 可以直接读取 manager 中的已连接服务器和工具快照
- 若某个服务器初始化失败，只影响该服务器，不阻断 CLI 主流程

**备选方案**:
- 首次调用工具时懒加载服务器：会让错误暴露滞后，也增加工具执行路径复杂度

### D6: 配置合并采用“按 server name 覆盖”策略

**选择**: `mcpServers` 是一个以 server name 为 key 的 map。user 与 project settings 合并时：
- 不同 server name 共存
- 同名 server 由更高优先级层（project）覆盖更低层（user）

**理由**:
- map 语义天然适合“命名服务器注册表”
- 相比列表拼接，覆盖规则更容易理解，也更适合项目局部重定义
- 与当前整体 settings 优先级体系一致

**备选方案**:
- 同名 server 视为错误：会增加用户迁移成本
- 同名 server 深度 merge：会造成字段来源难以推断

### D7: MCP 工具权限沿用现有 Tool 权限模型

**选择**: MCP 代理工具通过 `ToolInfo` 暴露名称、描述和 schema；权限检查仍走现有 `PermissionManager`。默认情况下，MCP 工具视为非只读、非并发安全，除非后续协议元数据明确可安全放宽。

**理由**:
- 安全优先。首期不假设远端工具是只读或并发安全
- 不需要修改现有权限系统架构，只需把 MCP 代理工具当成普通工具
- 后续可在协议元数据支持后再增加 `readOnlyHint` 映射

**备选方案**:
- 默认全部只读：存在明显安全风险
- 为 MCP 单独设计权限体系：会导致重复建模

### D8: `/mcp` 先提供文本快照，不做复杂 UI 状态面板

**选择**: 新增 `/mcp` slash command，输出服务器名、连接状态、工具数量和工具列表。TUI/print 模式都通过现有消息输出路径展示。

**理由**:
- 能满足可观测性需求，且实现简单
- 不阻塞核心协议与工具链路落地
- 后续如需状态栏或专门面板，可在该数据模型上继续扩展

## Risks / Trade-offs

**[协议兼容性风险]** → 自研 JSON-RPC/MCP 读写层可能与部分服务器实现存在细节兼容问题。  
→ **缓解**: 首期只对标准 stdio MCP server 做集成验证；协议层做最小实现并补充 framing/timeout/invalid JSON 测试。

**[服务器进程稳定性风险]** → MCP server 可能启动失败、异常退出或挂起，影响工具可用性。  
→ **缓解**: 初始化阶段隔离每个 server 的错误；单个 server 失败不阻塞主程序；调用时设置超时并返回明确错误。

**[动态工具安全风险]** → 外部工具能力未知，若默认权限过宽可能绕过用户预期。  
→ **缓解**: MCP 工具统一走现有权限系统，默认按高风险工具处理，不默认标记为只读。

**[系统 prompt 膨胀风险]** → 外部服务器工具过多时会显著扩大 prompt。  
→ **缓解**: 首期延续现有工具描述拼装策略；必要时对 MCP 工具描述数量或长度设置上限。

**[架构复杂度上升]** → 新增 crate 和动态注册流程会提高启动路径复杂度。  
→ **缓解**: 保持 manager API 简单：load config → connect servers → list tools → register proxies；不在本迭代引入重连与热更新。
