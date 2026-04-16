## Context

Rust Claude Code 已完成 14 个迭代，具备基础对话、工具执行、权限、TUI、compaction 能力。但每次 API 调用缺少两个 TypeScript 版本的基础设施级功能：

1. **Prompt Caching** — 当前每次请求都发送完整 system prompt（通常 >10K tokens），无缓存复用。TS 版本通过 `cache_control: { type: "ephemeral" }` 标记实现前缀缓存，连续对话可降低 ~90% 重复 token 成本。
2. **Extended Thinking** — 当前 `ContentBlock::Thinking` 类型已存在，但 API 请求中从未发送 `thinking` 配置字段，模型不会产生 thinking 输出。TS 版本默认开启 thinking，Opus/Sonnet 4.6 使用 adaptive 模式。
3. **Token 计数** — 当前使用 `chars / 4` 启发式，偏差大。TS 版本优先使用 API response 的 `usage` 数据。
4. **Max Tokens 恢复** — 当前 `stop_reason == MaxTokens` 时直接返回截断结果。TS 版本有 3 层恢复机制。

**关键类型现状**:
- `CreateMessageRequest` — 无 `thinking` 字段、无 `cache_control` 支持
- `SystemPrompt` — `Text(String)` | `Blocks(Vec<ContentBlock>)`，无结构化块
- `ContentBlock::Thinking` — 只有 `thinking: String`，缺少 `signature` 字段
- `CompactionConfig` — 使用 `estimate_message_tokens()` (chars/4)

## Goals / Non-Goals

**Goals:**
- 在 API 请求中正确设置 `cache_control` 标记，实现 prompt caching
- 支持 extended thinking（adaptive 和 budget-based 模式）
- Thinking 块增加 `signature` 字段，确保多轮对话正确性
- 改用 usage-based token 计数提升 compaction 精度
- Max tokens 截断时自动恢复继续生成
- TUI 中展示 thinking 状态和 cache hit 信息

**Non-Goals:**
- Cache break 检测与归因分析（TS 版本的 `promptCacheBreakDetection.ts`）— 复杂且非必需
- 1 小时 TTL cache（仅适用于 Anthropic 内部用户和特定订阅者）
- Anthropic `countTokens` API 集成 — 需要额外 HTTP 调用，性能开销大，usage-based 已足够
- Auto mode AI classifier — 这是独立的权限系统功能，不属于本迭代
- 非 streaming fallback — 当前架构暂不需要

## Decisions

### D1: System prompt 结构化块类型

**选择**: 新增 `SystemBlock` 类型，`SystemPrompt` 保留 `Text` 变体用于简单场景，新增 `StructuredBlocks` 变体。

```rust
pub struct SystemBlock {
    pub r#type: String,  // "text"
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_control: Option<CacheControl>,
}

pub struct CacheControl {
    pub r#type: String,  // "ephemeral"
}
```

**理由**: `SystemPrompt::Blocks(Vec<ContentBlock>)` 现有变体用于内容块（Text、Thinking 等），但 Anthropic API 的 system 字段需要的是 `{ type: "text", text: "...", cache_control: ... }` 格式的专用块，不是 ContentBlock。新增 `SystemBlock` 类型精确匹配 API schema。

**替代方案**: 在 `ContentBlock` 上添加 `cache_control` 字段 — 拒绝，因为 system 块的 schema 与 content 块不同。

### D2: 消息级 cache_control 注入位置

**选择**: 在 `build_request()` 中序列化消息时，在最后一条消息的最后一个内容块上注入 `cache_control`。

**理由**: TS 版本的策略是只放一个 cache marker 在最后一条消息上（注释中解释了为什么不放多个）。这最大化了可缓存的前缀长度。

**实现方式**: 不修改 `ContentBlock` 类型。在 `build_request()` 中将消息转为 `serde_json::Value`，然后在最后一个块上注入 `cache_control` 字段。这避免了污染核心类型。

### D3: Thinking 配置策略

**选择**: 三模式枚举 + 模型自动检测

```rust
pub enum ThinkingConfig {
    Disabled,
    Enabled { budget_tokens: u32 },
    Adaptive,
}
```

- Opus 4.6 / Sonnet 4.6 → Adaptive
- 其他支持 thinking 的模型 → Enabled with budget
- Claude 3.x → Disabled

**理由**: 与 TS 版本对齐。Adaptive 是最新模型的推荐模式，不需要指定 budget。旧模型需要显式 budget。

### D4: Thinking 块 signature 字段

**选择**: 在 `ContentBlock::Thinking` 中增加 `signature: Option<String>` 字段，使用 `#[serde(default, skip_serializing_if = "Option::is_none")]`。

**理由**: Anthropic API 要求在后续 turn 中回传 thinking 块时必须包含 `signature` 字段。没有 signature，API 会拒绝请求。`Option` 保持向后兼容（旧 session 文件没有 signature）。

### D5: Token 计数改进策略

**选择**: 双层计数 — usage-based 优先，chars/4 兜底。

1. `AppState` 追踪最近一次 API 响应的 `Usage` 数据
2. Compaction 阈值检查优先使用 `usage.input_tokens` 作为当前上下文大小的估算
3. 新增消息（未发送 API）使用 chars/4 增量估算
4. 公式: `last_api_input_tokens + estimate_new_messages_tokens()`

**理由**: API response 的 `input_tokens` 是服务端精确计数，最可靠。只对增量部分使用启发式。

### D6: Max tokens recovery 策略

**选择**: 简化版恢复 — 注入 continuation 消息重试，最多 3 次。

当 `stop_reason == MaxTokens` 时：
1. 保留当前截断的 assistant 消息
2. 注入 user 消息: `"Continue from where you left off. Do not repeat what you already said."`
3. 重新发送请求
4. 最多重试 3 次，超过后正常返回截断结果

**不实现**: TS 版本的 64K escalation（需要 GrowthBook feature flag）和 max_tokens 调整（需要解析错误消息提取 token 数）。

### D7: Forward-compatible content block 解析

**选择**: 在 `ContentBlock` 枚举添加 `#[serde(other)]` 兜底变体 `Unknown`。

**理由**: Anthropic API 可能返回新的块类型（如 `server_tool_use`），当前会导致反序列化失败。`Unknown` 变体让解析不会中断，未知块被安静忽略。

### D8: Streaming TUI 去重策略

**选择**: streaming 模式下，assistant 最终文本只通过 `StreamEnd` 落入消息列表；`main.rs` 在 `QueryLoop::run()` 返回后不再额外发送一次 `AssistantMessage`。non-streaming 模式仍保留显式发送最终 assistant 文本的逻辑。

**理由**: 实际验证中发现，TUI 在收到 `StreamEnd` 时已经会把 `streaming_text` 推入 `messages`，而 worker 线程在请求结束后再次调用 `send_assistant_message()`，导致用户看到两条完全相同的 assistant 消息。将最终落消息职责限定给 streaming UI 自身，可消除重复。

**替代方案**: 改成由 worker 统一负责最终落消息、TUI 在 `StreamEnd` 不再推入消息列表 — 拒绝，因为这会让 streaming UI 对 worker 结束时机产生额外耦合，并破坏当前 `StreamDelta`/`StreamEnd` 的自然模型。

## Risks / Trade-offs

**[R1] cache_control JSON 注入复杂性** → 在 `build_request()` 中通过 `serde_json::Value` 注入 cache_control，而非在类型系统中表达。这牺牲了类型安全换来了核心类型的简洁性。**Mitigation**: 封装为独立函数 `inject_cache_control()`，集中测试。

**[R2] Thinking signature 与旧 session 兼容性** → 旧 session 文件中的 Thinking 块没有 signature 字段。加载旧 session 后回传给 API 会被拒绝。**Mitigation**: `Option<String>` + `#[serde(default)]` 确保反序列化兼容。加载旧 session 时过滤掉无 signature 的 thinking 块（将其转为普通 text 块或丢弃）。

**[R3] Max tokens recovery 可能产生重复内容** → 模型可能在 continuation 中重复之前的内容。**Mitigation**: 恢复消息明确指示不重复。如果用户体验不佳，后续可通过更智能的 prompt 改进。

**[R4] 非 Anthropic API 端点可能不支持 cache_control 或 thinking** → 某些代理或第三方端点可能不支持这些字段。**Mitigation**: 如果 API 返回错误（400），检查是否为 cache_control 或 thinking 相关错误，如果是则在后续请求中降级关闭这些功能。首次实现不做自动降级，仅在文档中说明。

**[R5] streaming / non-streaming 双路径行为分叉** → 去重修复后，最终 assistant 文本在 streaming 与 non-streaming 两种模式下由不同位置落入 UI。**Mitigation**: 用注释明确边界，并保留现有测试；后续若重构 TUI 事件模型，可再统一这两条路径。