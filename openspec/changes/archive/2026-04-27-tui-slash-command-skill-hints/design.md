## Context

当前 `crates/tui` 已经具备输入缓冲区、历史浏览、slash command 分发、session picker、permission dialog 和 user question dialog 等交互结构，但输入框本身仍然是纯文本编辑区：用户输入 `/` 后既看不到可用命令，也无法在当前上下文中发现仓库已提供的 skills。现有 `Up` / `Down` 已被用于历史浏览或多行光标移动，`Tab` 已被用于 thinking 展开折叠，因此新增建议交互时必须明确优先级，避免破坏已有输入体验。

这个变更主要发生在 `crates/tui`，不会修改 QueryLoop、tool registry 或 skill tool 的执行协议。本轮重点是改善输入体验与能力可发现性，而不是引入新的 slash command 语法或真正从 TUI 直接执行 skill invocation。

## Goals / Non-Goals

**Goals:**
- 在输入首字符为 `/` 时显示 suggestion overlay，而不是要求用户先运行 `/help`
- 同时展示两类建议源：slash commands 与 skills，并在视觉上分组对齐
- 支持基于当前 `/` 后前缀的实时过滤
- 支持 `Up` / `Down` 选择和 `Enter` 应用候选项，同时尽量减少对现有历史浏览和提交语义的影响
- 在建议层不可见时保持现有输入、提交、历史浏览和 thinking 交互不变

**Non-Goals:**
- 不在本轮引入新的 slash command 执行协议来直接触发技能调用
- 不实现模糊匹配排序、分页搜索或鼠标点击选择
- 不把建议系统扩展到普通自然语言提示词、文件路径或 MCP 工具补全
- 不修改 QueryLoop 或 skill tool 的运行时能力边界

## Decisions

### D1: 使用输入框上方 overlay，而不是扩展输入框本身高度

**选择**: suggestion 列表渲染为锚定在输入框上方的轻量 overlay，独立于输入框正文区域。

**理由**: `draw_input_area` 当前假设逻辑行与视觉行一一对应，并据此计算 cursor 位置。如果把建议内容混入输入区，会让输入高度、cursor 映射和多行行为一起变复杂。overlay 可以复用现有 session picker / dialog 的渲染思路，同时不影响输入光标计算。

**替代方案**: 在输入框下方增加内嵌建议区 —— 拒绝，因为会挤压 chat viewport，并增加 input area height 与 cursor 布局耦合。

### D2: suggestion state 由 `App` 显式持有，并在每次输入变更后重算

**选择**: 在 `App` 中新增 suggestion state，至少包含是否可见、当前过滤 query、扁平化候选列表、选中索引，以及候选项的类型信息（command 或 skill）。状态在字符输入、删除、粘贴、Esc、Enter 应用后同步更新。

**理由**: suggestion 行为与现有 session picker、permission dialog 类似，都需要明确的交互状态。把它做成显式状态可以让键盘优先级和测试更稳定，也方便渲染层只消费已经整理好的候选项。

**替代方案**: 在 `ui.rs` 渲染时即席根据 `input_text()` 计算候选项 —— 拒绝，因为这样会把交互状态分散到渲染层，难以处理选中索引和按键测试。

### D3: slash 建议仅在输入首字符为 `/` 时激活，并优先处理 `Up`/`Down`/`Enter`

**选择**: 只有当输入第一字符是 `/` 时 suggestion overlay 才会出现；一旦可见且有候选项，`Up` / `Down` 优先移动建议选择，`Enter` 优先应用选中项到输入缓冲区，而不是提交消息。

**理由**: 用户已经明确要求 slash 场景下要有选择和补全。把导航优先级切到 suggestions 能避免和历史浏览冲突，并让交互可预测。

**替代方案**: 继续让 `Up` / `Down` 先用于历史浏览，只用额外快捷键选择建议 —— 拒绝，因为学习成本更高，也不符合典型 command palette 心智。

### D4: skills 作为只读建议源展示，不在本轮赋予可执行 slash 语义

**选择**: skills 与 commands 并列展示，但补全时仅把规范化名称插入输入框，不直接触发 skill 执行，也不新增 `/skill` 命令协议。

**理由**: 当前系统的技能触发发生在 agent 工具层，不存在已定义的用户侧 slash API。直接发明技能执行语法会把本轮输入体验改动扩大成命令协议设计。先提供发现和插入能力更符合最小变更原则。

**替代方案**: 为每个 skill 自动生成可执行 slash command —— 暂不采用，因为会牵涉命令注册、执行路径和权限/帮助文案语义变更。

### D5: 建议列表使用扁平化渲染模型，但带分组标题和稳定排序

**选择**: 内部候选项保持扁平化数组以便导航；渲染时按 `Commands`、`Skills` 分组输出标题，并对名称列使用固定宽度实现对齐。组内顺序保持 registry 顺序，过滤后不做额外重排。

**理由**: 扁平化数组最容易实现 `selected` 索引和 `Up` / `Down` 移动。渲染时再恢复分组，能兼顾实现简单和用户可读性。

**替代方案**: 用嵌套树结构同时维护组和索引 —— 拒绝，因为会增加导航逻辑复杂度，但收益有限。

## Risks / Trade-offs

- **[历史浏览回归]** → 仅在 suggestion overlay 可见且有候选项时拦截 `Up` / `Down`；其他情况保留现有 history 逻辑，并补回归测试
- **[技能列表与实际可用技能漂移]** → 第一版使用显式 registry，后续如引入动态技能发现再替换；当前设计文档和帮助文案需明确这是“可见建议集”
- **[overlay 挡住聊天末尾内容]** → 将 overlay 高度限制为输入区上方可用空间，并在候选项过多时裁剪显示窗口
- **[Enter 语义让用户困惑]** → 只有在 suggestion 可见且存在选中项时才应用候选；否则保留原有提交/命令执行行为
- **[命令与技能命名冲突]** → 渲染时始终显示分组标题，避免用户误把 skill 当作可直接执行的 slash command

## Migration Plan

- 无需数据迁移
- 新交互以向后兼容方式引入，已有 slash commands 名称和执行行为保持不变
- 若 suggestion state 计算失败或候选源为空，输入框退化为现有纯文本行为

## Open Questions

- skills 候选被应用到输入框时，最终插入纯技能名还是约定格式文本，是否需要在实现前再统一一次文案
- 第一版是否需要为没有候选项时显示 "No matching commands or skills" 空状态，还是直接隐藏 overlay
