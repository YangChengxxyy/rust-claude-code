## Why

长对话场景下，消息历史持续增长，导致 API 请求的 token 数量不断膨胀，最终会超过模型上下文窗口限制，导致对话中断或请求失败。当前 Rust 版本没有任何上下文压缩机制 —— 每次 API 请求都会发送完整的消息历史。这是日常重度使用的关键阻塞点，必须在 TUI/Git 增强之前解决。

## What Changes

- 新增 compaction 模块，支持将过长的消息历史压缩为语义摘要
- 支持手动 `/compact` 斜杠命令触发压缩
- 支持基于 token 阈值的自动压缩触发（在 QueryLoop 的每次 API 调用前检查）
- 压缩通过调用 LLM API 生成对话摘要，然后用摘要消息替换早期历史
- 保留最近 N 轮上下文不被压缩，确保对话连贯性
- TUI 中展示压缩状态反馈（正在压缩、压缩完成）
- 会话持久化保存压缩后的消息历史

## Capabilities

### New Capabilities
- `compaction-engine`: 核心压缩引擎 —— token 估算、阈值判定、摘要生成、消息替换逻辑
- `compact-command`: `/compact` 斜杠命令与 TUI 集成 —— 手动触发、自动触发、状态反馈

### Modified Capabilities

## Impact

- **crates/core**: 新增 `compaction.rs` 模块，定义 `CompactionConfig`、`CompactionResult` 等类型；可能扩展 `ContentBlock` 或 `Message` 以标记压缩摘要
- **crates/cli**: `QueryLoop` 在 `build_request()` 前增加压缩检查；新增 `compaction` 服务模块调用 API 生成摘要
- **crates/tui**: `AppEvent` 新增压缩相关事件；`app.rs` 新增 `/compact` 命令处理；TUI 展示压缩进度
- **crates/api**: 可能需要复用 `AnthropicClient` 发送摘要请求（无需新 API 端点，复用 `create_message`）
- **会话持久化**: `SessionFile` 保存压缩后的消息列表，压缩摘要在恢复会话时保留
