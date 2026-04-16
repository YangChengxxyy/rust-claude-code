## 1. Core 类型定义

- [x] 1.1 在 `core` crate 新增 `compaction.rs` 模块，定义 `CompactionConfig` 结构体（`context_window: u32`, `threshold_ratio: f32`, `preserve_ratio: f32`, `summary_max_tokens: u32`），实现 `Default` trait 和 serde 序列化
- [x] 1.2 在 `compaction.rs` 定义 `CompactionResult` 结构体（`original_message_count`, `compacted_message_count`, `preserved_message_count`, `estimated_tokens_before`, `estimated_tokens_after`, `summary_length`）
- [x] 1.3 在 `core/lib.rs` 导出 `compaction` 模块，为 `CompactionConfig` 和 `CompactionResult` 编写单元测试（默认值、serde 序列化/反序列化）

## 2. Token 估算与阈值检测

- [x] 2.1 在 `core/compaction.rs` 实现 `estimate_message_tokens(messages: &[Message]) -> u32` 函数，遍历所有 ContentBlock，将 text/JSON 内容的字符数除以 4 得到估算 token 数
- [x] 2.2 实现 `estimate_system_prompt_tokens(system_prompt: &Option<String>) -> u32` 函数
- [x] 2.3 实现 `needs_compaction(config: &CompactionConfig, system_prompt: &Option<String>, messages: &[Message]) -> bool` 函数，当总估算 token 超过 `context_window * threshold_ratio` 时返回 true
- [x] 2.4 编写 token 估算和阈值检测的单元测试，覆盖纯文本消息、含工具调用消息、阈值边界场景

## 3. 消息分区逻辑

- [x] 3.1 在 `core/compaction.rs` 实现 `partition_messages(config: &CompactionConfig, messages: &[Message]) -> (Vec<Message>, Vec<Message>)` 函数：从尾部开始累计，直到保留段估算 token 超过 `context_window * preserve_ratio`，其余为压缩段
- [x] 3.2 实现最小保留数量保护：保留段至少包含 2 条消息；消息总数 ≤ 4 条时返回全部保留不压缩
- [x] 3.3 编写分区逻辑的单元测试，覆盖正常分区、长最近消息、消息过少跳过等场景

## 4. 摘要生成服务

- [x] 4.1 在 `cli` crate 新增 `compaction.rs` 模块，定义 `CompactionService<C: ModelClient>` 结构体，持有 `client: C` 和 `config: CompactionConfig`
- [x] 4.2 定义压缩专用的 system prompt 常量 `COMPACTION_PROMPT`，指导 LLM 保留文件路径、工具调用摘要、关键决策和对话流程
- [x] 4.3 实现 `CompactionService::generate_summary()` 方法：构造 `CreateMessageRequest`，将待压缩消息作为 user 消息内容发送给 LLM，max_tokens 设为 `config.summary_max_tokens`，返回摘要文本
- [x] 4.4 实现 `CompactionService::compact()` 方法：调用 `needs_compaction()` → `partition_messages()` → `generate_summary()` → 替换 `AppState.messages`，返回 `CompactionResult`
- [x] 4.5 编写 `CompactionService` 的单元测试（使用 MockClient），覆盖成功压缩、API 失败、不需要压缩等场景

## 5. QueryLoop 集成自动压缩

- [x] 5.1 在 `QueryLoop` 中添加 `compaction_config: CompactionConfig` 字段和 `with_compaction_config()` builder 方法
- [x] 5.2 在 `QueryLoop::run()` 的主循环中，在 `build_request()` 调用前插入自动压缩检查：调用 `needs_compaction()`，若需要则执行 `CompactionService::compact()`
- [x] 5.3 自动压缩时通过 TuiBridge 发送 `CompactionStart` / `CompactionComplete` 事件
- [x] 5.4 编写 QueryLoop 自动压缩的集成测试（MockClient），验证超过阈值时自动触发压缩

## 6. TUI 事件与 /compact 命令

- [x] 6.1 在 `tui/events.rs` 的 `AppEvent` 枚举中新增 `CompactionStart` 和 `CompactionComplete { result: CompactionResult }` 变体
- [x] 6.2 在 `tui/bridge.rs` 的 `TuiBridge` 中新增 `send_compaction_start()` 和 `send_compaction_complete(result)` 方法
- [x] 6.3 在 `tui/app.rs` 的 `handle_slash_command()` 中添加 `/compact` 命令处理：通过 `user_tx` 发送 `[COMPACT_REQUEST]` 标记
- [x] 6.4 在 `tui/app.rs` 的 `/help` 命令输出中添加 `/compact` 说明
- [x] 6.5 在 `tui/app.rs` 的事件处理循环中处理 `CompactionStart` 和 `CompactionComplete` 事件，显示对应的系统消息
- [x] 6.6 编写 TUI 事件和 `/compact` 命令的单元测试

## 7. CLI 主入口集成

- [x] 7.1 在 `cli/main.rs` 的 TUI worker 中识别 `[COMPACT_REQUEST]` 标记，调用 `CompactionService::compact()` 而非 `QueryLoop::run()`
- [x] 7.2 在 worker 中压缩前后通过 bridge 发送 `CompactionStart` / `CompactionComplete` 事件
- [x] 7.3 在 `cli/main.rs` 的 `QueryLoop` 构建处传入 `CompactionConfig`
- [x] 7.4 确保非交互模式（`--print`）下的 `QueryLoop` 也正确配置了 `CompactionConfig`

## 8. 验证与回归测试

- [x] 8.1 运行 `cargo test --workspace` 确认所有现有测试和新增测试通过
- [x] 8.2 运行 `cargo check --workspace` 确认无编译警告
- [x] 8.3 运行 `cargo clippy --workspace` 确认无 lint 问题（无新增 warning）
- [ ] 8.4 手动验证：在 TUI 中输入 `/compact` 确认命令可用且显示正确状态消息
