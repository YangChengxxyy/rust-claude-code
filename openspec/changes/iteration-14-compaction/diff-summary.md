# iteration-14 当前 diff 整理

> 记录时间：2026-04-16
> 范围：当前工作区未提交改动（compaction 相关实现）

## 1. 当前改动概览

本轮 diff 主要完成了 **长对话上下文压缩（compaction）** 的一整条链路，并顺手补齐了与模型选择、usage 持久化、TUI 状态反馈相关的支撑改动。

从当前工作区状态看：

- 已修改文件：11 个
- 新增 Rust 源文件：3 个
  - `crates/cli/src/compaction.rs`
  - `crates/core/src/compaction.rs`
  - `crates/core/src/model.rs`
- 新增 OpenSpec 变更目录：`openspec/changes/iteration-14-compaction/`

`git diff --stat` 当前摘要：

- 11 个已跟踪文件变更
- 约 `758 insertions / 684 deletions`
- 另有 3 个新 Rust 文件与 1 个新的 OpenSpec 变更目录尚未纳入该统计

## 2. 本轮实现的核心能力

### 2.1 对话压缩引擎

新增 `crates/core/src/compaction.rs`：

- 定义 `CompactionConfig`
  - `context_window`
  - `threshold_ratio`
  - `preserve_ratio`
  - `summary_max_tokens`
- 定义 `CompactionResult`
  - 原消息数
  - 被压缩消息数
  - 保留消息数
  - 压缩前后估算 token
  - 摘要长度
- 实现 token 粗略估算：基于 `chars / 4`
- 实现是否需要压缩的阈值判断
- 实现消息分区逻辑：
  - 优先保留最近消息
  - 至少保留 2 条消息
  - 总消息数过少时跳过压缩
- 已带配套单元测试

### 2.2 摘要生成服务

新增 `crates/cli/src/compaction.rs`：

- 引入 `CompactionService<C: ModelClient>`
- 通过同一个模型 API 生成摘要，而不是本地截断
- 定义独立 `COMPACTION_PROMPT`，要求保留：
  - 文件路径
  - 工具调用与结果
  - 关键决策
  - 错误与处理过程
  - 当前工作状态
- 压缩后的历史以一条普通 user message 替换，格式为：
  - `Message::user("[COMPACTED]\n\n{summary}")`
- 提供：
  - `compact()`
  - `force_compact()`
  - `compact_if_needed()`
- 已带 MockClient 单元测试

### 2.3 QueryLoop 自动压缩

`crates/cli/src/query_loop.rs` 已接入自动压缩：

- `QueryLoop` 新增 `compaction_config`
- 提供 builder：`with_compaction_config()`
- 在每轮 `build_request()` 前执行自动压缩检查
- 构造请求时不再直接盲用 `state.session.model`，而是根据运行时条件重新计算模型
- assistant 消息现在保留 `usage` 并写入 state，供后续逻辑使用

### 2.4 手动 `/compact` 命令

`crates/tui/src/app.rs` 与 `crates/cli/src/main.rs` 已接入手动压缩：

- TUI 新增 `/compact`
- `/help` 中已补充 `/compact` 说明
- TUI 通过发送特殊标记 `[COMPACT_REQUEST]` 给 worker 触发压缩
- worker 收到后直接调用 `CompactionService::force_compact()`
- 压缩完成后会保存 session

### 2.5 TUI 压缩状态反馈

已补齐 TUI 事件与桥接：

- `crates/tui/src/events.rs`
  - 新增 `CompactionStart`
  - 新增 `CompactionComplete { result }`
- `crates/tui/src/bridge.rs`
  - 新增 `send_compaction_start()`
  - 新增 `send_compaction_complete()`
- `crates/tui/src/app.rs`
  - 收到开始事件时显示：`Compacting conversation history...`
  - 收到完成事件时显示压缩结果摘要

### 2.6 会话与 usage 持久化增强

为支撑 compaction 和运行时模型切换，本轮还补了状态与 session 相关改动：

- `crates/core/src/message.rs`
  - `Message` 已支持 `usage: Option<Usage>` 序列化
  - 新增 `assistant_with_usage()`
- `crates/core/src/state.rs`
  - `SessionSettings` 新增 `model_setting`
  - `AppState` 新增/完善：
    - `add_assistant_message()`
    - `most_recent_assistant_usage()`
- `crates/cli/src/session.rs`
  - `SessionFile` 新增 `model_setting`
  - 兼容老 session：若缺失 `model_setting`，回填为 `model`
  - session roundtrip 测试已覆盖 assistant usage 持久化

### 2.7 模型选择逻辑抽离

新增 `crates/core/src/model.rs`：

- 统一模型 alias 解析：
  - `opus`
  - `sonnet`
  - `haiku`
  - `best`
  - `opusplan`
- 提供 `normalize_model_string_for_api()`，在发请求前去掉 `[1m]` / `[2m]` 后缀
- 提供 `usage_exceeds_200k_tokens()`
- 提供 `get_runtime_main_loop_model()`
  - `opusplan + Plan mode + 未超过 200k` → `claude-opus-4-6`
  - `opusplan + Plan mode + 超过 200k` → `claude-sonnet-4-6`
  - `haiku + Plan mode` → `claude-sonnet-4-6`

这一块不仅服务 compaction，也顺便把运行时模型选择从 `main.rs` / `query_loop.rs` 中抽了出来。

## 3. 文件级别整理

### core

- `crates/core/src/lib.rs`
  - 导出 `compaction` 与 `model` 模块
- `crates/core/src/compaction.rs`
  - 新增：压缩配置、结果、token 估算、分区逻辑、测试
- `crates/core/src/model.rs`
  - 新增：模型 alias、runtime model 决策、API model 规范化
- `crates/core/src/message.rs`
  - `Message` 增强 usage 持久化能力
- `crates/core/src/state.rs`
  - session/model setting/usage 查询能力增强

### cli

- `crates/cli/src/lib.rs`
  - 导出 `compaction`
- `crates/cli/src/compaction.rs`
  - 新增：摘要服务与 compaction 实现
- `crates/cli/src/query_loop.rs`
  - 自动压缩接入
  - assistant usage 入库
  - 请求模型改为 runtime model 计算
- `crates/cli/src/main.rs`
  - 配置解析新增 `model_setting`
  - TUI worker 支持 `[COMPACT_REQUEST]`
  - `QueryLoop` 构建时注入 `CompactionConfig`
  - `continue session` 时恢复 `model_setting`
- `crates/cli/src/session.rs`
  - session 文件兼容扩展

### tui

- `crates/tui/src/events.rs`
  - compaction 事件新增
- `crates/tui/src/bridge.rs`
  - compaction bridge 方法新增
- `crates/tui/src/app.rs`
  - `/compact` 命令接入
  - `/help` 文案更新
  - compaction 事件处理接入
- `crates/tui/src/ui.rs`
  - 当前 diff 有小幅调整，主体仍是现有渲染层

### OpenSpec

- `openspec/changes/iteration-14-compaction/proposal.md`
  - 说明为何要做 compaction，以及能力范围
- `openspec/changes/iteration-14-compaction/design.md`
  - 记录设计决策与取舍
- `openspec/changes/iteration-14-compaction/tasks.md`
  - 当前大部分任务已勾选完成
  - 唯一未勾选项为：`8.4 手动验证 /compact`

## 4. 与 OpenSpec 任务对照的当前状态

根据 `openspec/changes/iteration-14-compaction/tasks.md` 当前内容：

- 1 ~ 7 节任务已全部勾选完成
- 8.1 / 8.2 / 8.3 也被标记为已完成
- 仅剩：
  - `8.4 手动验证：在 TUI 中输入 /compact 确认命令可用且显示正确状态消息`

需要注意：

- 本次“整理 diff”过程**没有重新执行** `cargo test --workspace`、`cargo check --workspace`、`cargo clippy --workspace`
- 因此这里仅记录 **OpenSpec 任务清单上的当前勾选状态**，不额外替代一次新的验证结论

## 5. 当前 diff 呈现出的实现特征

### 已形成闭环的部分

1. **自动压缩闭环**
   - 触发判断 → 生成摘要 → 替换历史 → 继续发请求

2. **手动压缩闭环**
   - `/compact` → worker 特殊分支 → compaction service → session 保存

3. **状态闭环**
   - usage 持久化 → 可据最近 assistant usage 决定 runtime model

4. **恢复闭环**
   - session 恢复后仍能带回 `model_setting` 与 assistant usage

### 本轮 diff 中值得关注的点

1. **自动压缩与手动压缩的 UI 事件时机不完全一致**
   - 手动 `/compact` 路径会先发 `CompactionStart`，再执行压缩
   - `QueryLoop` 自动压缩路径当前是在 `compact_if_needed()` 返回成功后，才顺序发送 `CompactionStart` 与 `CompactionComplete`
   - 这意味着自动压缩时，TUI 的“开始压缩”提示更像是结果通知的一部分，而不是一个真实的“进行中”状态

2. **压缩摘要仍是普通 user text message**
   - 优点：侵入性小，不需要改 API 协议层
   - 代价：后续若要做特殊渲染或更强语义识别，仍需要依赖 `[COMPACTED]` 前缀

3. **token 估算仍是启发式**
   - 当前实现明确接受误差，用阈值余量规避风险
   - 这是设计内的已知 trade-off，而不是遗漏

## 6. 建议的下一步

如果下一步继续推进，我建议优先做这两件事：

1. **完成 8.4 手动验证**
   - 实际进入 TUI
   - 连续制造较长消息历史
   - 执行 `/compact`
   - 验证状态提示与 session 保存是否符合预期

2. **决定是否修正自动压缩事件时机**
   - 如果希望 TUI 真正展示“正在压缩”，应把 `CompactionStart` 放到摘要请求发起之前
   - 当前实现功能上可用，但交互语义略滞后

## 7. 一句话总结

当前 diff 基本已经把 iteration-14 的 compaction 主体能力做完了：**core 有压缩规则，cli 有摘要服务与自动压缩，tui 有 `/compact` 与状态提示，session 与 usage 也补齐了恢复与 runtime model 支撑；剩下最明确的尾项是手动联调验证。**
