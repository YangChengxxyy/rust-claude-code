## Context

当前 Rust Claude Code 的每次 API 请求都发送完整的 `AppState.messages` 历史。长对话场景下 token 数持续膨胀，最终会超出模型上下文窗口（200K tokens），导致请求失败。

现有架构中：
- `QueryLoop::build_request()` 将 `state.messages` 全量转为 `ApiMessage` 发送
- `AppState.total_usage` 仅记录累计 token 数（input/output/cache），无法反推当前历史的 token 占用
- `Usage` 来自 API 响应，每次响应的 `input_tokens` 即为当次请求的 prompt token 数
- `SessionFile` 保存完整 messages 列表，无压缩概念
- TUI 的 `/clear` 仅清空显示，不影响消息历史

原版 Claude Code 的 compaction 策略：在 token 占用超过模型上下文窗口约 80% 时，调用 LLM 将早期历史压缩为一条摘要消息，保留最近若干轮上下文。

## Goals / Non-Goals

**Goals:**

- 实现基于 token 阈值的自动压缩触发机制
- 实现手动 `/compact` 命令触发压缩
- 通过调用同一 LLM API 生成对话摘要
- 压缩后的消息结构可被 QueryLoop 正常消费
- 压缩后的会话可正常持久化和恢复
- TUI 中展示压缩状态反馈

**Non-Goals:**

- 精确的 token 计数（本迭代使用基于字符的估算，不引入 tokenizer 依赖）
- 增量式 micro-compact（每轮压缩一点）—— 留待后续迭代
- 压缩前的完整历史备份/回溯能力
- session memory compact（跨会话记忆压缩）
- 可配置的压缩策略选择器

## Decisions

### 1. Token 估算方式：基于字符的粗略估算

**选择**: 使用 `chars / 4` 作为 token 估算公式。

**备选方案**:
- 引入 `tiktoken-rs` 做精确计数 —— 增加编译依赖，对 Rust 项目来说 tiktoken 绑定质量不稳定
- 使用 API 响应中的 `input_tokens` 作为参考 —— 只有在发送请求后才知道，无法提前判断

**理由**: 粗略估算足以判断"是否接近上下文上限"，误差 20% 在此场景可接受。后续可替换为更精确的实现。

### 2. 压缩触发阈值：基于估算 token 数

**选择**: 当估算 prompt token 数（system prompt + messages）超过 `context_window * 0.8` 时自动触发。默认 context_window = 200,000。

**理由**: 80% 是原版 Claude Code 使用的阈值，留出足够余量给 assistant 响应和新用户输入。

### 3. 摘要生成方式：复用 ModelClient 调用 LLM

**选择**: 构造一个特殊的 `CreateMessageRequest`，将待压缩的历史消息 + 压缩指令作为 prompt 发送给同一模型，获取摘要文本。

**备选方案**:
- 使用本地摘要算法（如文本截断 + 关键信息提取）—— 质量不足，无法保留语义
- 使用专门的小模型做摘要 —— 增加配置复杂度

**理由**: 复用已有 `ModelClient` 最简洁，质量最高，无需新增依赖。摘要请求的 max_tokens 设为 8192，足够生成详细摘要。

### 4. 压缩后的消息结构

**选择**: 用一条 `Message::user([ContentBlock::Text { text: "[COMPACTED] ..." }])` 替换被压缩的早期消息。不新增 ContentBlock 变体。

**备选方案**:
- 新增 `ContentBlock::CompactionSummary` 变体 —— 改动面大，需要修改 serde、API 转换、TUI 渲染等
- 注入 system prompt —— 污染 system prompt 语义，且每次重建 system prompt 都需要考虑

**理由**: 使用普通 Text 消息 + `[COMPACTED]` 前缀最小化改动。API 端将其视为普通 user 消息，无需修改序列化。TUI 端可通过前缀识别并特殊渲染。

### 5. 保留最近上下文的策略

**选择**: 压缩时保留最近的消息，直到保留部分的估算 token 数不超过 `context_window * 0.5`。即：压缩后总 token 预算 = 摘要 + 保留消息 ≈ context_window 的 50-60%。

**理由**: 保留足够的最近上下文以维持对话连贯性，同时给后续对话留出空间。

### 6. 压缩模块放置位置

**选择**: 在 `cli` crate 新增 `compaction.rs` 模块，包含 `CompactionService` 结构体。类型定义（`CompactionConfig`、`CompactionResult`）放在 `core` crate。

**理由**: 压缩逻辑需要调用 `ModelClient`，而 `ModelClient` trait 定义在 `cli` crate。类型放 `core` 供其他 crate 引用。

### 7. `/compact` 命令的触发路径

**选择**: TUI 中的 `/compact` 命令通过向 worker 发送特殊标记（通过 `user_tx` channel 发送 `[COMPACT_REQUEST]`）触发。worker 收到后调用 `CompactionService::compact()` 而非 `QueryLoop::run()`。

**备选方案**:
- 在 TUI 层直接调用压缩 —— TUI 不持有 `ModelClient`，无法直接调用
- 新增专用 channel —— 增加复杂度

**理由**: 复用现有的 user_tx channel 最简洁。worker 端通过前缀识别即可区分正常输入和压缩请求。

## Risks / Trade-offs

- **[估算不准确]** → 字符/4 的估算对中文内容偏差较大（中文 token 通常 1-2 字符/token）。缓解：阈值设在 80%，留足余量。后续可根据 API 返回的 `input_tokens` 动态校准。

- **[摘要丢失关键信息]** → LLM 摘要可能遗漏某些工具调用细节或文件路径。缓解：压缩 prompt 明确要求保留文件路径、工具调用摘要和关键决策。

- **[压缩过程中 API 调用额外消耗 token]** → 生成摘要本身需要发送被压缩的历史给 LLM。缓解：只有在接近上下文上限时才触发，这是必要的开销。

- **[压缩与并发 QueryLoop 的竞态]** → 自动压缩在 QueryLoop 中同步执行（在 build_request 前），不存在并发问题。手动 `/compact` 在 worker 中顺序处理。

- **[压缩后会话恢复]** → 恢复的会话包含压缩摘要消息，模型可能不完全理解 `[COMPACTED]` 前缀的含义。缓解：在 system prompt 中不特殊处理，依赖模型的上下文理解能力。
