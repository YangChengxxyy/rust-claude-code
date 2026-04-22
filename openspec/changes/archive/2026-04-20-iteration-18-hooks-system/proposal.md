## Why

Hooks 是 Claude Code 的核心自动化扩展机制，允许用户在关键生命周期节点（工具执行前后、用户提交输入、会话停止等）自动触发自定义脚本。当前 Rust 版本缺乏任何扩展点，用户无法在工具执行前进行自定义安全检查、日志记录、输入修改或工作流自动化。Hooks 系统也是后续 MCP 集成和插件生态的基础设施。

## What Changes

- 新增 `HookEvent` 枚举，定义核心 hook 事件类型（PreToolUse、PostToolUse、UserPromptSubmit、Stop、Notification）
- 新增 `HookConfig` 配置模型，从 `settings.json` 的 `hooks` 字段加载 hook 定义
- 新增 `HookRunner`，负责匹配事件、执行 shell command、解析结果
- Hook 通过 stdin 接收 JSON 格式的上下文输入，通过 stdout 返回 JSON 决策结果
- PreToolUse hook 支持 `approve` / `block` 决策，可阻止工具执行或修改工具输入
- 集成到 QueryLoop：工具执行前触发 PreToolUse、工具执行后触发 PostToolUse、用户输入时触发 UserPromptSubmit
- 集成到 Settings 系统：从 user/project 级 `settings.json` 加载并合并 hook 配置
- Hook 执行支持超时保护，不阻塞主流程

## Capabilities

### New Capabilities
- `hook-config`: Hook 配置模型定义与 settings.json 加载，包括 matcher 匹配、hook 类型声明和配置合并
- `hook-execution`: Hook 执行引擎 — shell command 执行、JSON stdin/stdout 通信、超时处理、结果解析
- `hook-integration`: Hook 与 QueryLoop/TUI 的集成 — PreToolUse/PostToolUse/UserPromptSubmit/Stop 事件触发点

### Modified Capabilities
<!-- No existing capabilities are modified at spec level -->

## Impact

- **core crate**: 新增 hook 类型定义（`HookEvent`、`HookConfig`、`HookResult`）
- **cli crate**: 新增 `HookRunner` 模块，QueryLoop 集成 hook 调用
- **tui crate**: Hook 执行状态显示（可选），`/hooks` slash command 显示已配置的 hooks
- **settings.json**: 新增 `hooks` 字段，格式与 TS 版本兼容
- **依赖**: 无新外部依赖（shell 执行使用 `tokio::process::Command`，JSON 使用 `serde_json`）
