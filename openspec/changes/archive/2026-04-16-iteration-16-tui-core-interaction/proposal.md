## Why

当前 TUI 已具备基础聊天、权限对话框、Todo 面板与 slash commands，但日常交互仍有明显断点：单行输入无法顺畅粘贴多行代码、历史输入不可回溯、Markdown 输出可读性差、流式输出缺少可中断控制。这些问题直接影响实际可用性，因此需要把下一轮重点放在 TUI 核心交互体验上。

## What Changes

- 重构输入系统，支持多行编辑、代码粘贴、输入历史浏览，以及更完整的光标/按词移动快捷键
- 增强消息渲染，补齐 Markdown 基础显示能力，包括标题、列表、代码块、行内代码、粗体与斜体
- 为流式会话补齐常用交互控制：`Escape` 取消当前流式输出、`Ctrl+C` 在不同上下文下执行取消或退出、`Ctrl+L` 清屏
- 扩展 thinking 块 UI，在已有 spinner/折叠摘要基础上增加展开查看入口与交互
- 为输入历史增加本地持久化，确保重启后仍可浏览近期输入

## Capabilities

### New Capabilities
- `tui-input-experience`: 多行输入、粘贴检测、历史浏览、光标与按词移动等输入体验能力
- `tui-markdown-rendering`: TUI 中对标题、列表、代码块、行内代码、粗体、斜体等 Markdown 元素的基础渲染能力
- `tui-stream-controls`: 流式输出期间的取消、清屏及相关快捷键行为

### Modified Capabilities
- `extended-thinking`: thinking 块在 TUI 中从“仅显示 spinner/折叠摘要”扩展为“可折叠并支持显式展开查看”

## Impact

- **`crates/tui`**: `app.rs`、`ui.rs`、`events.rs`、`bridge.rs` 将承担主要改动，涉及输入缓冲、历史管理、快捷键分发、渲染与 thinking 交互状态
- **`crates/cli`**: 需要与 TUI 的流式取消机制对接，保证取消当前输出不会破坏 QueryLoop 生命周期
- **本地配置/数据目录**: 新增或扩展历史文件存储（`~/.config/rust-claude-code/history`）
- **依赖**: 可能引入 Markdown/语法高亮相关依赖（如 `syntect`），并为终端兼容性补充测试
