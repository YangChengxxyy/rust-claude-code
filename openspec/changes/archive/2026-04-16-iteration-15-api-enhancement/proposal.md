## Why

当前 Rust 版本的每次 API 调用都缺少两个 TypeScript 版本的基础设施级功能：Prompt Caching（可降低 90% 重复 token 成本）和 Extended Thinking（Opus/Sonnet 4.6 的深度推理能力）。这两项影响每一次请求的成本和质量，是后续所有迭代的基础。此外，长输出被 max_tokens 截断时缺少自动恢复机制，导致复杂任务频繁中断。

## What Changes

- **Prompt Caching**: 在 system prompt 块和最后一条消息上设置 `cache_control: { type: "ephemeral" }`，实现前缀缓存复用，大幅降低连续对话成本
- **Extended Thinking**: API 请求中注入 `thinking` 配置，支持 adaptive（Opus/Sonnet 4.6）和 budget-based 两种模式，默认开启
- **Thinking 块完整性**: `ContentBlock::Thinking` 增加 `signature` 字段，确保多轮对话中 thinking 块可正确回传
- **Token 计数改进**: 优先使用 API response 的 usage 数据进行 token 计数，chars/4 仅作兜底，提升 compaction 阈值判定精度
- **Max Tokens Recovery**: 当 `stop_reason == MaxTokens` 时自动注入恢复消息继续生成（最多 3 次重试）
- **TUI Thinking 展示**: 流式显示 thinking spinner，完成后折叠为摘要行
- **Cache Hit 状态显示**: TUI 状态栏展示 cache hit 比例

## Capabilities

### New Capabilities
- `prompt-caching`: System prompt 和消息级别的 cache_control 标记，实现 Anthropic prompt caching
- `extended-thinking`: API 请求中的 thinking 配置（adaptive/budget-based），thinking 块 signature 支持
- `max-tokens-recovery`: 输出被 max_tokens 截断时的自动恢复机制（注入 continuation 消息重试）
- `usage-based-token-counting`: 基于 API response usage 数据的 token 计数，替代 chars/4 启发式

### Modified Capabilities
- `compaction-engine`: token 计数从 chars/4 切换为 usage-based，阈值判定精度提升

## Impact

- **`api` crate**: `CreateMessageRequest` 新增 `thinking` 字段；`SystemPrompt` 类型重构为支持 `cache_control` 的结构化块；`ContentBlock::Thinking` 增加 `signature` 字段；响应解析需要 forward-compatible 处理未知块类型
- **`core` crate**: `SessionSettings` 新增 thinking 配置；token 计数逻辑重构；`CompactionConfig` 适配新计数
- **`cli` crate**: `QueryLoop` 请求构建逻辑增加 cache_control 和 thinking 注入；新增 max tokens recovery 循环；CLI 参数增加 `--thinking`/`--no-thinking`
- **`tui` crate**: thinking 块渲染增强；状态栏增加 cache hit 信息
- **API 兼容性**: 需要确保 `cache_control` 和 `thinking` 字段在非支持端点（如第三方代理）下 graceful fallback

## Implementation Notes

- 已通过真实 CLI 请求验证 prompt caching 生效：相同请求第二次调用时 `cache_read_input_tokens` 明显大于 0，且 `input_tokens` 显著下降
- 已通过真实 CLI 请求验证 thinking 生效：响应中出现 `type: "thinking"` 内容块，且带有 `signature`
- 实现过程中发现并修复了一个 TUI 流式展示问题：assistant 文本在 `StreamEnd` 后已落入消息列表，`main.rs` 又额外发送一次 `AssistantMessage`，导致同一条 assistant 消息重复显示。修复方式是在 streaming 模式下避免第二次派发最终 assistant 文本，仅保留 non-streaming 模式的显式派发