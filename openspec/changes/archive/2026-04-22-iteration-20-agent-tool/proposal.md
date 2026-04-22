## Why

当前 Rust Claude Code 缺乏子代理能力，复杂任务只能在单个 QueryLoop 内线性处理。原版 Claude Code 的 AgentTool 允许模型 spawn 独立子代理处理复杂子任务（如"去另一个文件做完整重构"），是处理工程级多步骤任务的核心能力。此外，现有的 TodoWriteTool 仅支持平面待办列表，缺少正式的 Task 管理模型（带状态机、所有者追踪），无法支撑子代理任务分配和进度汇报。

## What Changes

- 新增 `AgentTool`：从 QueryLoop 内部 spawn 独立子代理（独立消息历史、共享 cwd 和权限配置），执行完毕后将最终文本作为工具结果返回
- 新增 `Task` 数据模型和 `TaskStore`：支持 create/list/update/get 操作，状态流转 pending → in_progress → completed
- 用 Task 系列工具替换现有 `TodoWriteTool`：`TaskTool` 统一处理 create/list/update/get 操作
- 扩展 `ToolContext` 以携带创建子代理所需的依赖（API 客户端工厂、工具注册表模板、hook runner 引用）
- TUI Todo panel 扩展为 Task panel，展示 task 状态和子代理进度

## Capabilities

### New Capabilities
- `agent-tool`: AgentTool 实现——从工具执行上下文中 spawn 独立 QueryLoop 子代理，支持工具子集过滤和递归深度限制
- `task-system`: Task 数据模型、TaskStore 内存存储、TaskTool（create/list/update/get 子命令）、与 TUI 的集成

### Modified Capabilities
<!-- No existing spec-level requirements are changing -->

## Impact

- **core crate**: 新增 `Task`、`TaskStatus`、`TaskStore` 类型，替换现有 `TodoItem`/`TodoStatus`
- **tools crate**: 新增 `AgentTool`、`TaskTool`，移除 `TodoWriteTool`；`ToolContext` 扩展新字段
- **cli crate**: `QueryLoop` 需要支持从 `ToolContext` 内部被 spawn；`main.rs` 需向 `ToolContext` 注入客户端工厂和工具注册表
- **tui crate**: Todo panel 改为 Task panel，渲染 `Task` 列表和状态图标
- **依赖**: 无新外部依赖
