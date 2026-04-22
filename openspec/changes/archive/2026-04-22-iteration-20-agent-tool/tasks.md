## 1. Task 数据模型与存储（core crate）

- [x] 1.1 在 `core` crate 新增 `Task`、`TaskStatus`（Pending/InProgress/Completed/Cancelled）、`TaskPriority` 类型
- [x] 1.2 在 `AppState` 中将 `todos: Vec<TodoItem>` 替换为 `tasks: Vec<Task>`，并更新 `update_todos` 为 `update_tasks` 方法
- [x] 1.3 保持 `TodoItem`/`TodoStatus`/`TodoPriority` 类型为已弃用兼容别名（type alias 到 Task 类型），确保 TUI 编译通过
- [x] 1.4 补充 Task 类型的序列化/反序列化和 AppState 操作的单元测试
- [x] 1.5 验证 `cargo test -p rust-claude-core` 通过

## 2. AgentContext 与 ToolContext 扩展

- [x] 2.1 在 `tools` crate 的 `ToolContext` 中新增 `agent_context: Option<AgentContext>` 字段
- [x] 2.2 定义 `AgentContext` 结构体：`client: Arc<dyn ModelClient>`、`tool_registry_factory: Arc<dyn Fn() -> ToolRegistry + Send + Sync>`、`hook_runner: Option<Arc<HookRunner>>`、`current_depth: u32`、`max_depth: u32`
- [x] 2.3 将 `ModelClient` trait 从 `cli` crate 提升到 `api` crate（或提取到 `core`），使 `tools` crate 可以引用
- [x] 2.4 为 `ToolContext` 的 Default 实现更新，确保 `agent_context` 默认为 `None`
- [x] 2.5 验证 `cargo test -p rust-claude-tools` 通过（现有工具不受影响）

## 3. TaskTool 实现（tools crate）

- [x] 3.1 新增 `TaskTool`，input schema 包含 `command`（create/list/update/get）及对应子字段
- [x] 3.2 实现 `create` 子命令：生成唯一 ID、创建 Task、存入 AppState、返回任务详情
- [x] 3.3 实现 `list` 子命令：读取 AppState 中所有 tasks、格式化为文本列表返回
- [x] 3.4 实现 `update` 子命令：按 ID 查找 task、更新 status/content/priority、返回更新后状态
- [x] 3.5 实现 `get` 子命令：按 ID 查找 task、返回详细信息
- [x] 3.6 实现错误处理：无效 command、task 不存在、缺少必填字段
- [x] 3.7 移除 `TodoWriteTool` 注册，改为注册 `TaskTool`（更新 lib.rs 导出）
- [x] 3.8 补充 TaskTool 各子命令的单元测试
- [x] 3.9 验证 `cargo test -p rust-claude-tools` 通过

## 4. AgentTool 实现（tools crate）

- [x] 4.1 新增 `AgentTool`，input schema 包含 `prompt`（必填）和 `allowed_tools`（可选）
- [x] 4.2 实现 execute：从 ToolContext 获取 AgentContext，检查递归深度限制
- [x] 4.3 创建子代理 AppState：继承 cwd、permission_mode、rules、session，清空 messages
- [x] 4.4 构建子代理 ToolRegistry：使用 tool_registry_factory，根据 allowed_tools 过滤
- [x] 4.5 创建并运行子代理 QueryLoop（max_rounds=5），等待完成
- [x] 4.6 提取子代理最终文本和 token usage，格式化为 ToolResult 返回
- [x] 4.7 处理子代理执行错误，将错误信息包装为 ToolResult error
- [x] 4.8 标记 AgentTool 为非只读、非并发安全
- [x] 4.9 补充 AgentTool 的单元测试（使用 MockClient：深度限制、无 AgentContext、正常执行）
- [x] 4.10 验证 `cargo test -p rust-claude-tools` 通过

## 5. CLI 集成（cli crate）

- [x] 5.1 将 `ModelClient` trait 移至 `api` crate 或 `core` crate，更新 `query_loop.rs` 引用
- [x] 5.2 在 `main.rs` 中创建 `AgentContext`，注入 `Arc<dyn ModelClient>`、工具工厂闭包、hook_runner
- [x] 5.3 在 QueryLoop 的工具执行路径中将 `AgentContext` 传入 `ToolContext`
- [x] 5.4 更新 `build_tools()` 注册 `TaskTool` 和 `AgentTool`（替代 `TodoWriteTool`）
- [x] 5.5 确认 system prompt 中 AgentTool 和 TaskTool 的描述正确生成
- [x] 5.6 补充 CLI 层测试：AgentContext 注入、工具注册、子代理 spawn
- [x] 5.7 验证 `cargo test -p rust-claude-cli` 通过

## 6. TUI Task panel 适配

- [x] 6.1 将 TUI 的 `AppEvent::TodoUpdate` 改为接受 `Vec<Task>`（或保持 alias 兼容）
- [x] 6.2 更新 `app.rs` 中 todo panel 的数据源，使用 Task 类型
- [x] 6.3 更新 `ui.rs` 中 `draw_todo_panel` 渲染逻辑，支持 Cancelled 状态图标
- [x] 6.4 将面板标题从 "Todo" 改为 "Tasks"
- [x] 6.5 验证 `cargo test -p rust-claude-tui` 通过

## 7. 集成验证

- [x] 7.1 运行 `cargo test --workspace` 确认全仓通过
- [x] 7.2 运行 `cargo build` 确认编译成功
- [x] 7.3 验证 AgentTool 可被 QueryLoop 调用并返回子代理结果（集成测试）
- [x] 7.4 验证 TaskTool 的 create/list/update/get 在 QueryLoop 中正常工作
- [x] 7.5 更新 `doc/requirement.md` 中迭代 20 的状态
