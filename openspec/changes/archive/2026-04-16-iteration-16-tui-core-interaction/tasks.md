## 1. 输入缓冲与历史系统

- [x] 1.1 重构 `crates/tui/src/app.rs` 的输入模型，从单行字符串升级为支持多行内容与二维光标定位的缓冲结构
- [x] 1.2 实现 `Shift+Enter` 插入换行、`Enter` 提交整段输入，并补充多行提交与换行编辑测试
- [x] 1.3 实现多行粘贴保真，确保换行与缩进在输入缓冲和提交结果中保持不变
- [x] 1.4 增加输入历史内存模型，支持 `Up` / `Down` 浏览历史，并在历史浏览与当前草稿之间正确切换
- [x] 1.5 实现 `~/.config/rust-claude-code/history` 的历史持久化加载与保存，并补充损坏记录/重启恢复测试
- [x] 1.6 实现 `Home`、`End`、`Ctrl+A`、`Ctrl+E`、`Ctrl+Left`、`Ctrl+Right` 等编辑快捷键并补充行为测试

## 2. Markdown 渲染增强

- [x] 2.1 在 `crates/tui/src/ui.rs` 中引入 Markdown 预解析层，将消息拆分为标题、列表、段落、代码块和行内样式片段
- [x] 2.2 实现标题与有序/无序列表的基础样式和缩进渲染，并补充渲染单元测试
- [x] 2.3 实现 fenced code block 的独立块渲染，保证空白符、换行和无高亮降级路径正确
- [x] 2.4 实现行内代码、粗体、斜体的样式渲染，并验证与普通文本混排时不破坏换行/滚动

## 3. 流式控制与 thinking 交互

- [x] 3.1 为 TUI 与后台 worker 增加流式取消通道，支持 `Escape` 取消当前 streaming 输出且不退出应用
- [x] 3.2 调整 `Ctrl+C` 行为：流式期间执行取消，空闲状态执行退出，并补充对应事件流测试
- [x] 3.3 实现 `Ctrl+L` 清屏/重绘可视区域，确保当前会话消息和输入草稿仍然保留
- [x] 3.4 在 `crates/tui/src/app.rs` 与 `crates/tui/src/ui.rs` 中增加 completed thinking block 的展开/折叠状态管理与快捷键交互
- [x] 3.5 补充 thinking 展开、折叠、流式取消后的 UI 回退测试

## 4. 集成验证

- [x] 4.1 运行 `cargo test -p rust-claude-tui`
- [x] 4.2 运行 `cargo test -p rust-claude-cli -p rust-claude-tui`
- [x] 4.3 手动运行 `cargo run -p rust-claude-cli` 验证多行粘贴、历史浏览、Markdown 渲染、流式取消与 thinking 展开交互
