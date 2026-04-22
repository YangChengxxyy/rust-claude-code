## Context

当前 Rust Claude Code 具备完整的 QueryLoop 工具执行流水线（消息构建 → API 调用 → 工具执行 → 结果追加 → 循环），但所有交互都在单一会话内线性进行。若模型需要"去完成一个子任务"（例如在另一个文件做重构、运行测试并修复所有问题），当前的单循环模型会导致主会话上下文膨胀。

原版 Claude Code 通过 `AgentTool` 解决此问题：model 调用 `AgentTool` 时，会 fork 出一个独立的 QueryLoop 子代理，拥有独立消息历史但共享 cwd 和权限，子代理执行完毕后将最终文本返回给主循环。

现有状态：
- `QueryLoop<C>` 持有 `client: C`、`tools: ToolRegistry`、`max_rounds`、`bridge`、`hook_runner`
- `ToolContext` 仅包含 `tool_use_id` 和 `Option<Arc<Mutex<AppState>>>`
- `TodoWriteTool` 使用 `AppState.todos: Vec<TodoItem>` 存储平面待办
- `ModelClient` trait 已抽象，允许 mock 测试
- TUI 通过 `AppEvent::TodoUpdate` 接收 todo 变更

## Goals / Non-Goals

**Goals:**
- 实现 `AgentTool`，能从工具执行上下文中 spawn 独立 QueryLoop 子代理
- 子代理拥有独立消息历史，但继承 cwd、权限模式和 API 凭证
- 子代理支持工具子集过滤（调用时指定 `allowed_tools`）
- 支持递归深度限制防止无限嵌套
- 用正式的 Task 模型替换 TodoWriteTool 的平面待办列表
- TaskTool 统一暴露 create/list/update/get 子命令
- TUI Task panel 展示 task 状态

**Non-Goals:**
- 不实现子代理的异步/并行执行（子代理同步阻塞直到完成）
- 不实现子代理间的通信或任务依赖关系
- 不实现 task 的持久化存储（仅内存）
- 不实现 task 的 owner/blocked_by 等高级字段（保留简单的状态机）
- 不实现子代理独立的 TUI 渲染面板（结果通过父循环工具结果展示）

## Decisions

### D1: 通过扩展 ToolContext 注入子代理依赖

**选择**: 在 `ToolContext` 中新增 `agent_context: Option<AgentContext>` 字段，`AgentContext` 包含创建子代理所需的最小依赖：API 客户端引用、工具注册表模板、当前递归深度。首版不在子代理中继承 hooks。

**理由**:
- 最小侵入性——现有工具不受影响（`agent_context` 是 `Option`）
- 保持 `Tool` trait 签名不变
- 统一管理子代理所需的最小依赖

**备选方案**:
- 在 ToolRegistry 中存放客户端引用：会破坏 registry 的纯工具注册职责
- 使用全局 service container：不符合当前显式依赖注入的代码风格

### D2: AgentContext 使用 trait object 持有 API 客户端

**选择**: `AgentContext` 持有 `Arc<dyn ModelClient>` 而非具体类型。`QueryLoop::new` 已经接受泛型 `C: ModelClient`，但在 `ToolContext` 中需要类型擦除以避免泛型传染。

**理由**:
- `ToolContext` 是非泛型结构体，嵌入泛型 `C` 会导致 `Tool` trait 和 `ToolRegistry` 全部泛型化
- `Arc<dyn ModelClient>` 运行时开销可忽略（仅虚表跳转）
- 子代理的 `QueryLoop` 可以直接接收 `Arc<dyn ModelClient>` 作为客户端

**备选方案**:
- 将 ToolContext 泛型化：泛型传染影响面过大，需要改动 Tool trait 和所有工具实现
- 用闭包工厂：更复杂，没有明显好处

### D2.1: 子代理执行通过回调注入，而不是直接依赖 QueryLoop

**选择**: `AgentContext` 不直接暴露 `QueryLoop`，而是携带一个由 `cli` 注入的子代理执行回调。`AgentTool` 只负责参数校验、深度检查和调用回调；回调内部由 `cli` 构建并运行子代理 `QueryLoop`。

**理由**:
- `QueryLoop` 当前位于 `cli` crate，而 `AgentTool` 位于 `tools` crate，直接调用会导致循环依赖
- 回调式注入能保持 crate 分层不变：`cli -> tools` 单向依赖继续成立
- `AgentTool` 保持为纯工具适配层，子代理运行细节仍由 CLI 层控制

**备选方案**:
- 将 `QueryLoop` 下沉到更低层 crate：改动面大，不适合当前迭代中途进行
- 将 `AgentTool` 挪到 `cli` crate：与当前 proposal/spec 的工具分层不一致

### D3: 子代理创建独立 AppState，不共享父消息历史

**选择**: AgentTool 在执行时为子代理创建新的 `AppState`，从父 state 继承 `cwd`、`permission_mode`、`always_allow/deny_rules`、`session`（model、max_tokens、stream 等），但 `messages` 从空开始。

**理由**:
- 子代理的职责是独立完成一个子任务，不需要父会话的上下文
- 避免子代理的 token 消耗因为继承大量父消息而暴涨
- 子代理的系统提示从父 state 继承

**备选方案**:
- 共享 messages：会导致子代理和父循环的消息混杂
- 传递 messages 的摘要：增加复杂度，首期不需要

### D4: 递归深度通过 AgentContext 追踪，默认限制为 3 层

**选择**: `AgentContext` 包含 `current_depth: u32` 和 `max_depth: u32`（默认 3）。子代理创建时 `current_depth += 1`。当 `current_depth >= max_depth` 时，AgentTool 拒绝执行并返回错误。

**理由**:
- 简单有效地防止无限递归
- 3 层已足够应对绝大多数场景（主代理 → 子任务 → 更小子任务）
- 不需要子代理注册表中移除 AgentTool（通过深度限制即可）

**备选方案**:
- 从子代理的工具列表中去掉 AgentTool：过于保守，阻止了合理的嵌套场景
- 无限制：有栈溢出和 token 耗尽的风险

### D5: Task 系统替换 TodoWriteTool，使用统一的 TaskTool

**选择**: 新增 `TaskTool`，通过 input 中的 `command` 字段区分操作（create/list/update/get）。移除 `TodoWriteTool`。`AppState.todos` 字段类型改为 `Vec<Task>`，`Task` 包含 `id`、`content`、`status`（Pending/InProgress/Completed/Cancelled）、`priority`。

**理由**:
- 单个工具比四个独立工具更节省 system prompt 空间
- 保持向后兼容的字段映射（Task 的 content/status/priority 与 TodoItem 高度对应）
- 模型已经习惯了 TodoWriteTool 的"传入完整列表"模式，TaskTool 的 list/update 子命令提供更灵活的操作

**备选方案**:
- 四个独立工具（TaskCreate/TaskList/TaskUpdate/TaskGet）：模型需要学习四个工具名，system prompt 膨胀
- 保留 TodoWriteTool 并在旁边新增 Task 系列：两套 API 并存，会混淆

### D6: AgentTool 不注入 TuiBridge，子代理结果仅通过工具返回值传递

**选择**: 子代理的 QueryLoop 不设置 bridge，不向 TUI 发送流式事件。子代理执行完毕后，AgentTool 将最终文本作为 ToolResult 返回给父循环，由父循环的正常工具结果展示路径呈现。

**理由**:
- 大幅简化首期实现，避免 bridge 事件的嵌套和前缀管理
- 父循环会将子代理的返回文本当作正常工具结果展示
- 后续迭代可以增加 bridge 代理转发

**备选方案**:
- 注入代理 bridge 并前缀事件：复杂度高，TUI 侧需要处理嵌套缩进
- 通过 AppState 事件通道传递：增加新的跨模块通信机制

### D7: 首版子代理不继承 hooks

**选择**: `AgentTool` 创建的子代理在 iteration 20 中不继承父代理的 `HookRunner`。子代理仅复用 cwd、权限、model/session 配置和工具集。

**理由**:
- `HookRunner` 当前位于 `cli` crate，而 `AgentTool` 位于 `tools` crate，直接传递会导致循环依赖
- 首期目标是先打通子代理主链路：spawn QueryLoop -> 执行任务 -> 返回结果
- hooks 继承可以作为后续独立迭代处理，不阻塞当前功能落地

**备选方案**:
- 将 `HookRunner` 下沉到更低层 crate：架构更完整，但改动面明显扩大
- 为 hooks 增加共享 trait 抽象：长期更合理，但超出本迭代的最小闭环范围

## Risks / Trade-offs

**[Token 消耗风险]** → 子代理每次执行都是完整的 API 对话，可能迅速消耗大量 token。
→ **缓解**: 子代理默认 `max_rounds` 为 5（低于主循环的 8），且在 AgentTool 返回结果中附带 token 使用量汇总。

**[递归深度风险]** → 模型可能在子代理中再次调用 AgentTool，导致链式嵌套。
→ **缓解**: `max_depth` 默认 3，超限时返回明确错误提示模型直接完成任务。

**[状态一致性风险]** → 子代理和父代理同时修改文件系统可能冲突。
→ **缓解**: 当前设计中子代理是同步阻塞的（父循环等待子代理完成），不存在并发修改。

**[TodoWriteTool 迁移风险]** → 移除 TodoWriteTool 可能影响现有 system prompt 中对 TodoWrite 的引用。
→ **缓解**: TaskTool 的描述中明确说明它替代了 TodoWriteTool；模型会通过 system prompt 中的工具列表适应新工具名。

**[子代理 hooks 缺失风险]** → 子代理执行路径与父代理在 hooks 行为上暂时不完全一致。
→ **缓解**: 在本迭代内明确将 hooks 继承列为非目标，优先保证 AgentTool 主链路可用；后续可单独抽象 hooks 能力。
