## Why

当前 TUI 已经支持多行输入、slash commands 和若干交互弹层，但输入框在用户输入 `/` 后仍缺少即时提示能力。随着内置命令增多、技能数量增加，用户需要先记住完整命令或离开输入流查看 `/help`，这会降低可发现性并打断操作节奏。

## What Changes

- 为 TUI 输入框增加 slash-triggered suggestion overlay，在输入以 `/` 开头时展示可用命令与技能提示
- 对建议项按当前 `/` 后前缀进行实时过滤，并把命令与技能分为独立分组显示
- 增加键盘交互：`Up`/`Down` 在建议项之间移动，`Enter` 应用当前选中项到输入框
- 保持现有非 slash 输入、历史浏览、普通提交和 thinking 折叠行为在未显示建议时不变

## Capabilities

### New Capabilities
<!-- None -->

### Modified Capabilities
- `tui-input-experience`: 输入框在 slash 前缀下增加建议浮层、过滤和选择交互
- `slash-command-extensions`: slash command 发现与补全体验扩展为可交互建议列表，并与技能提示并列展示

## Impact

- **`crates/tui/src/app.rs`**: 增加 suggestion state、过滤结果与按键优先级处理
- **`crates/tui/src/ui.rs`**: 增加输入框上方建议浮层渲染、分组标题和对齐布局
- **测试**: 需要补充 suggestion 过滤、导航、应用和现有历史/提交不回归的单元测试
- **用户体验**: slash commands 与技能更易发现，但不改变底层 QueryLoop 或 tool 执行模型
