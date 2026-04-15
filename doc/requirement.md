# Rust Claude Code — 需求与迭代计划

## 1. 项目目标

基于 Claude Code 的 TypeScript 源码（已通过 sourcemap 还原），开发一个 Rust 版本的 Claude Code，具备基础的对话、编辑、规划能力。

本文档承担两类职责：

- 作为参考源码的需求摘要，说明 Claude Code 原始设计的关键能力边界。
- 作为 Rust 实现的迭代主计划，记录当前已完成内容与后续迭代目标。

### 参考源码

- 路径: `/Users/yangchengxxyy/projects/claude-code-sourcemap/restored-src/src/`
- 核心文件：`Tool.ts`、`tools.ts`、`Task.ts`、`query.ts`、`claude.ts`、`toolOrchestration.ts`、`AppState.tsx`、`permissions.ts`
- 工具实现：`BashTool`、`FileReadTool`、`FileEditTool`、`FileWriteTool`、`TodoWriteTool`、`EnterPlanModeTool`

### 文档约定

- 第 3 节描述参考源码中的设计结论，不等同于当前 Rust 仓库的实现状态。
- 当前 Rust 实现状态以第 4 节为准。
- 与官方设计的差距和取舍，统一记录在 `doc/iteration-3-alignment.md`。

---

## 2. 技术选型

| 决策项 | 选型 | 说明 |
|--------|------|------|
| TUI 框架 | ratatui | 成熟的 Rust TUI 库 |
| LLM API | Anthropic API（直接对接） | 不使用中间层 |
| 工具范围 | Core 5 | Bash, FileRead, FileEdit, FileWrite, TodoWrite |
| 权限系统 | 完整权限模式 | default, acceptEdits, bypassPermissions, plan, dontAsk |
| 项目结构 | Cargo workspace | 多 crate: core, api, tools, tui, cli |

---

## 3. 参考源码分析

本节用于提炼 Claude Code 原始实现的设计意图，作为 Rust 版本的对齐依据。

### 3.1 消息系统 (`types/message.js`)

- 消息由 role 与 content blocks 组成。
- ContentBlock 至少包含：Text、ToolUse、ToolResult、Thinking。
- ToolUse 包含 id、name、input（JSON）。
- ToolResult 包含 tool_use_id、content、is_error。
- system prompt 在整体对话协议中占据独立位置，Rust 版本是否建模为独立 role 需要结合 API 设计取舍。

### 3.2 Tool 系统 (`Tool.ts`, `tools.ts`)

- 每个工具实现统一的 Tool 接口。
- 核心方法：`call()`、`checkPermissions()`、`inputSchema`、`isReadOnly()`、`isConcurrencySafe()`。
- 工具通过工厂函数构建，并由注册表按名称统一索引。

### 3.3 查询循环 (`query.ts`)

- 核心 agent 循环是一个 async generator。
- 循环逻辑：发送消息 -> 流式接收 -> 检测 `tool_use` -> 执行工具 -> 拼接 `tool_result` -> 继续循环。
- 支持最大轮次限制。
- `tool_use` 结果会自动写回消息历史。

### 3.4 工具调度 (`services/tools/toolOrchestration.ts`)

- 工具分为并发安全和非并发安全两类。
- 并发安全工具可以并行执行。
- 非并发安全工具需要串行执行。
- 调度层统一负责工具分组与执行顺序。

### 3.5 权限系统 (`types/permissions.ts`)

- 5 种权限模式：`default`、`acceptEdits`、`bypassPermissions`、`plan`、`dontAsk`。
- 支持 always_allow / always_deny 规则。
- 规则可以附带 pattern，例如 `Bash(git *)`。
- 权限系统不仅负责静态规则匹配，也要为交互确认和持久化留出边界。

### 3.6 核心工具实现

#### BashTool

- 支持超时、工作目录设置。
- 包含危险命令检测。
- 输出需要截断与格式化。

#### FileEditTool

- 基于 `old_string` -> `new_string` 的查找替换模式。
- 默认要求匹配唯一，除非 `replace_all = true`。
- 空 `old_string` + 非空 `new_string` 表示创建新文件。

#### FileReadTool

- 支持 `offset` / `limit` 分页读取。
- 返回带行号前缀的输出。
- 支持目录列表模式。

#### FileWriteTool

- 支持创建或覆盖文件。
- 自动创建父目录。

#### TodoWriteTool

- 更新 AppState 中的 todo 列表。
- 每个 todo 包含 `content`、`status`、`priority`。
- 全部 `completed` 时清空列表。

### 3.7 AppState

- AppState 是会话级状态容器。
- 至少需要承载消息历史、todo、权限相关状态、模型与会话配置、工作目录等信息。
- TypeScript 实现使用 React 上下文；Rust 版本最终可采用状态对象加异步协调机制，不要求直接复制 React 结构。

---

## 4. 当前实现状态

本节只描述当前仓库中的实际落地情况，用于与参考源码和后续迭代目标区分。

### 4.1 core crate

- 已实现 `Message`、`ContentBlock`、`Role`、`StopReason`、`Usage`。
- `Role` 当前只包含 `User` 和 `Assistant`。
- 已实现 `ToolResult`、`ToolResultMetadata`、`ToolInfo`。
- 已实现 `PermissionMode`、`PermissionRule`、`PermissionCheck`，并收敛了统一权限检查入口与规则优先级边界。
- 已实现 `AppState`、`SessionSettings`、`TodoItem`、`TodoStatus`、`TodoPriority`。
- 已实现 `Config`，支持从配置文件或 `ANTHROPIC_API_KEY` 加载 API Key。

### 4.2 api crate

- 已实现非流式 `AnthropicClient`。
- 已支持基于 `reqwest` 的 `POST /v1/messages` 调用。
- 已支持 `x-api-key` 和 `anthropic-version` 请求头。
- 已实现基础错误映射：认证失败、限流、通用 API 错误、超时、连接错误。
- 已实现 `CreateMessageRequest`、`CreateMessageResponse`、`ApiMessage`、`ApiTool`、`SystemPrompt`。
- `CreateMessageRequest` 已支持 `metadata`，`AnthropicClient` 已收敛共享的 header / request / JSON 响应处理边界。
- 已实现 SSE 流式基础设施：`MessageStream`、`StreamEvent`、SSE 事件解析、真实流式请求入口与 `examples/streaming_chat.rs`。
- 已实现基础 delta 累积器，支持 `text_delta`、`thinking_delta` 与 `input_json_delta` 还原完整内容块。
- 已新增 `examples/simple_chat.rs` 与真实 API 的忽略型集成测试。

### 4.3 tools crate

- 已实现 `Tool` trait、可执行的 `ToolRegistry` 与首批 5 个核心工具。
- `BashTool` 已支持 shell 执行、timeout、workdir、危险命令检测与输出截断。
- 已实现 `FileReadTool`、`FileEditTool`、`FileWriteTool`、`TodoWriteTool`，并完成工具注册与基础测试覆盖。

### 4.4 cli crate

- 当前为最小入口程序，已接入配置加载与 `AppState` 初始化的占位路径。
- 尚未接入 Query Loop、参数解析和交互模式。

### 4.5 tui crate

- 当前仅有最小 crate 骨架。
- 尚未实现界面状态、渲染和交互逻辑。

---

## 5. 设计边界说明

本节记录当前已知的设计边界，避免把初版设想误当作当前定稿。

### 5.1 Message 与 ContentBlock

当前 Rust 版本采用 `role + content blocks` 的消息模型，已覆盖文本、工具调用、工具结果与 thinking block。

已知待收敛点：

- `system` 内容当前主要通过 API 请求层的 `system` 字段承载，而不是 `Role::System`。
- `ContentBlock::ToolResult` 与 `tool_types::ToolResult` 的建模边界仍需在迭代 3 中进一步对齐。

### 5.2 Tool 接口

参考设计中的 Tool 接口已经明确，但 Rust 版本目前尚未在 `tools` crate 中正式落地。后续实现应至少覆盖以下能力：

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value;
    fn is_read_only(&self) -> bool;
    fn is_concurrency_safe(&self) -> bool;
    async fn call(&self, input: serde_json::Value, state: &mut AppState) -> ToolResult;
    fn check_permissions(&self, input: &serde_json::Value, state: &AppState) -> PermissionCheck;
}
```

### 5.3 Permission 系统

当前 Rust 版本已经实现权限模式和规则匹配的基础类型，但尚未实现 `PermissionManager`、规则持久化与 Query Loop 集成。

### 5.4 AppState

当前 `AppState` 已包含以下字段：

- `messages`
- `todos`
- `permission_mode`
- `model`
- `max_tokens`
- `cwd`
- `total_usage`

后续是否需要引入更明确的会话配置对象或权限上下文，将在迭代 3 的对齐文档中统一说明。

---

## 6. 项目结构设计

```text
rust-claude-code/
├── Cargo.toml              # workspace 根配置
├── doc/
│   ├── requirement.md      # 总需求与迭代计划
│   └── iteration-3-alignment.md
├── crates/
│   ├── core/               # 核心类型、消息系统、状态管理
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── message.rs
│   │       ├── tool_types.rs
│   │       ├── permission.rs
│   │       ├── state.rs
│   │       └── config.rs
│   ├── api/                # Anthropic API 客户端
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── client.rs
│   │       ├── types.rs
│   │       ├── stream.rs
│   │       └── error.rs
│   ├── tools/              # 工具实现
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── registry.rs
│   │       ├── bash.rs
│   │       ├── file_read.rs
│   │       ├── file_edit.rs
│   │       ├── file_write.rs
│   │       └── todo_write.rs
│   ├── tui/                # TUI 界面
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── app.rs
│   │       ├── chat_view.rs
│   │       ├── input.rs
│   │       ├── permission_dialog.rs
│   │       └── todo_panel.rs
│   └── cli/                # CLI 入口
│       ├── Cargo.toml
│       └── src/
│           └── main.rs
```

---

## 7. 迭代计划

### 迭代 1：项目骨架 + 消息类型系统

**状态**: 已完成

**完成记录**:

- 已创建 Cargo workspace 和 5 个 crate 骨架：`core`、`api`、`tools`、`tui`、`cli`
- 已实现 `core` 基础类型：`Message`、`ContentBlock`、`Role`、`StopReason`、`Usage`
- 已实现 `core` 状态与配置类型：`AppState`、`TodoItem`、`Config`
- 已实现 `core` 权限基础类型：`PermissionMode`、`PermissionRule`、`PermissionCheck`
- 已实现 `api` 基础请求/响应类型：`CreateMessageRequest`、`CreateMessageResponse`、`ApiMessage`、`SystemPrompt`
- 已实现 `tools` 的基础 `ToolRegistry` 存根和 `cli` / `tui` 的最小入口文件
- 已补充 serde 序列化/反序列化及基础行为测试
- 已修正 3 个迭代 1 阶段发现的问题：
  - `Config::load()` 支持配置文件缺失 `api_key` 时回退到 `ANTHROPIC_API_KEY`
  - 权限规则支持通过 `check_tool_with_command()` 处理带 `pattern` 的命令匹配
  - `CreateMessageRequest.system` 支持文本和结构化内容块两种形式
- 验证结果：`cargo build` 通过，`cargo test` 通过

**目标**: 创建 Cargo workspace，定义所有核心类型。

**产出**:

- Cargo workspace（5 个 crate）
- `core` crate: Message, ContentBlock, Role, StopReason 类型
- `core` crate: Tool trait, ToolUse, ToolResult 类型
- `core` crate: PermissionMode, PermissionRule 类型
- `core` crate: AppState struct
- `api` crate: API 请求/响应类型（CreateMessageRequest, CreateMessageResponse）

**验收标准**:

- `cargo build` 全部 crate 编译通过
- `cargo test` 通过，覆盖类型的 serde 序列化/反序列化测试
- Message 能正确序列化为 Anthropic API 格式的 JSON

---

### 迭代 2：Anthropic API 客户端（非流式）

**状态**: 已完成

**完成记录**:

- 已实现 `api` crate 的 `AnthropicClient`
- 已支持基于 `reqwest` 的非流式 `POST /v1/messages` 调用
- 已支持 `x-api-key` 与 `anthropic-version` 请求头注入
- 已实现基础错误映射：认证失败、限流、通用 API 错误、超时、连接错误
- 已补充 `CreateMessageRequest` / `CreateMessageResponse` 相关单元测试
- 已新增 `examples/simple_chat.rs` 示例程序
- 已新增基于 `ANTHROPIC_API_KEY` 的忽略型真实 API 集成测试
- 已修复示例程序在仅返回非文本 content block 时输出为空的问题
- 验证结果：`cargo test -p rust-claude-api` 通过，`cargo build -p rust-claude-api --example simple_chat` 通过

**目标**: 实现基础的 Anthropic API 客户端，支持非流式 `create_message` 调用。

**产出**:

- `api` crate: AnthropicClient struct
- HTTP 请求构造（reqwest）
- API Key 认证
- 错误处理（APIError 类型，rate limit, auth error 等）
- 非流式 `create_message` 方法

**验收标准**:

- 集成测试：能用真实 API Key 发送一条消息并收到回复
- 单元测试：请求序列化正确，错误类型覆盖完整
- 示例程序（`examples/simple_chat.rs`）：终端交互式对话

**依赖**: 迭代 1

---

### 迭代 3：官方设计对齐（迭代 1 + 2）

**状态**: 进行中

**定位**:

这是一个设计收敛迭代，不以新增终端用户可见功能为主，而是用来校准迭代 1 和迭代 2 形成的公共边界。其产出将作为迭代 4、迭代 7 和迭代 8 的约束。

**目标**:

- 对照 Claude Code 原始实现，补齐当前 Rust 版本在消息模型、权限模型、AppState、API 客户端上的关键差距记录。
- 明确哪些设计已经对齐，哪些只是部分对齐，哪些要延后到后续迭代处理。
- 在不破坏现有测试的前提下，为流式传输、Query Loop、权限系统预留稳定接口边界。

**产出**:

- 一份差距记录文档：`doc/iteration-3-alignment.md`
- `core` crate: 复核基础类型设计
  - 复核 `Message` / `ContentBlock` / `ToolResult` / `Usage` 的字段与行为
  - 复核 `PermissionMode` / `PermissionRule` 的建模边界
  - 复核 `AppState` 的最小必要字段
- `api` crate: 复核客户端边界
  - 梳理请求构造、metadata、header、base_url / provider 扩展点
  - 为后续流式与重试逻辑预留稳定接口
- 文档中明确记录本轮不处理的差距，避免提前引入过度抽象

**验收标准**:

- 文档中明确记录当前 Rust 实现与原始 Claude Code 的差距和取舍
- `core` 与 `api` 的公共类型在不破坏现有测试的前提下完成必要调整
- 为后续流式传输和 Query Loop 预留的接口边界清晰，不再依赖临时兼容设计
- `cargo test` 保持通过

**依赖**: 迭代 2

---

### 迭代 4：SSE 流式传输

**状态**: 已完成

**目标**: 支持 Anthropic API 的 SSE 流式响应。

**产出**:

- `api` crate: SSE 事件解析器
- Delta 累积器：将 `content_block_delta` 事件累积为完整的 `ContentBlock`
- MessageStream：返回 `Stream<Item = StreamEvent>` 的异步流
- StreamEvent 枚举：`MessageStart`、`ContentBlockStart`、`ContentBlockDelta`、`ContentBlockStop`、`MessageDelta`、`MessageStop`

**验收标准**:

- 单元测试：SSE 文本解析正确（event, data 字段提取）
- 单元测试：Delta 累积正确（多个 text delta -> 完整 text）
- 集成测试：流式接收一条完整消息，逐 token 打印
- 示例程序（`examples/streaming_chat.rs`）：实时打印 token

**依赖**: 迭代 3

---

### 迭代 5：Tool 系统框架 + BashTool

**状态**: 已完成

**目标**: 实现 Tool trait、ToolRegistry，并完成第一个工具 BashTool。

**产出**:

- `tools` crate: ToolRegistry（按名称注册/查找工具）
- `tools` crate: BashTool 实现
  - 执行 shell 命令（`tokio::process::Command`）
  - 支持 timeout 参数
  - 支持 workdir 参数
  - 输出截断（超长输出时截取首尾）
  - 危险命令检测（`rm -rf /`、`sudo` 等）
- 工具输入验证（基于 `input_schema`）

**验收标准**:

- 单元测试：BashTool 执行简单命令（`echo`、`ls`）返回正确输出
- 单元测试：超时功能（执行 `sleep` 命令验证超时）
- 单元测试：工作目录切换
- 单元测试：危险命令检测
- 单元测试：ToolRegistry 注册/查找/列出工具

**依赖**: 迭代 1

---

### 迭代 6：FileRead + FileEdit + FileWrite + TodoWrite

**状态**: 已完成

**目标**: 实现剩余 4 个核心工具。

**产出**:

- `tools` crate: FileReadTool
  - 读取文件内容，支持 offset/limit
  - 行号前缀
  - 目录列表模式
- `tools` crate: FileEditTool
  - `old_string` -> `new_string` 替换
  - 唯一匹配检查
  - `replace_all` 选项
  - 创建新文件（空 `old_string`）
- `tools` crate: FileWriteTool
  - 创建/覆盖文件
  - 自动创建父目录
- `tools` crate: TodoWriteTool
  - 更新 AppState 中的 todo 列表
  - 状态管理（pending/in_progress/completed）
  - 全部完成时清空

**验收标准**:

- FileReadTool：读取已知文件，验证行号和内容正确；分页读取；目录列出
- FileEditTool：单次替换、多次替换（`replace_all`）、唯一性检查失败、创建新文件
- FileWriteTool：写入新文件、覆盖已有文件、自动创建目录
- TodoWriteTool：添加/更新/清空 todo，状态转换
- 所有工具注册到 ToolRegistry

**依赖**: 迭代 5

---

### 迭代 7：查询循环（Query Loop）

**状态**: 已完成

**目标**: 实现核心 agent 循环，串联 API 调用和工具执行。

**产出**:

- `cli` crate: QueryLoop struct
  - 发送消息到 API
  - 流式接收响应
  - 检测 `tool_use` blocks
  - 执行工具（尊重并发安全性）
  - 拼接 `tool_result` 到消息历史
  - 继续循环直到 `stop_reason != tool_use`
- 工具调度逻辑
  - 并发安全工具并行执行（`tokio::join!`）
  - 非并发安全工具串行执行
- 最大轮次限制
- CLI 入口程序（`cli` crate `main.rs`）：stdin 读取用户输入，调用 QueryLoop

**验收标准**:

- 集成测试：发送包含 `tool_use` 的对话，验证工具被正确调用
- 集成测试：多轮工具调用（API 连续返回 `tool_use`）
- 集成测试：并发工具执行（多个 FileRead 并行）
- CLI 可运行：`cargo run -- "list files in current directory"` 正确调用 BashTool

**依赖**: 迭代 4, 迭代 6

**当前对齐说明**:

- 当前 `QueryLoop` 放在 `cli` crate，而不是 `core` crate，以避免 `core -> api` 与现有 `api -> core` 形成循环依赖。
- 流式响应消费、工具调用回填、多轮工具调用与并发安全工具并发执行已实现。
- CLI 已接入最小 QueryLoop 路径，并补充了基于本地兼容端点的 ignored 集成验证（支持通过环境变量覆盖 `base_url`、认证方式与模型）。

---

### 迭代 8：权限系统

**目标**: 实现完整的 5 种权限模式。

**产出**:

- `core` crate: PermissionManager
  - 权限检查逻辑
  - always_allow / always_deny 规则管理
  - 规则持久化（`~/.config/rust-claude-code/permissions.json`）
- 5 种权限模式的完整行为：
  - `default`: 写操作需确认
  - `acceptEdits`: 文件编辑自动允许，Bash 需确认
  - `bypassPermissions`: 全部允许
  - `plan`: 拒绝所有写操作
  - `dontAsk`: 拒绝需确认的操作
- QueryLoop 集成权限检查

**验收标准**:

- 单元测试：每种模式对每种工具的权限判定正确
- 单元测试：always_allow/always_deny 规则匹配（含 pattern）
- 单元测试：规则持久化/加载
- CLI 支持 `--mode` 参数切换权限模式

**依赖**: 迭代 7

---

### 迭代 9：TUI 基础框架

**目标**: 用 ratatui 构建基础 TUI 界面。

**产出**:

- `tui` crate: App struct（TUI 主循环）
- 布局：
  - 顶部状态栏（模型名称、权限模式、token 用量）
  - 中间聊天区域（消息列表，支持滚动）
  - 底部输入框（多行输入，支持 Enter 发送、Shift+Enter 换行）
- 消息渲染：
  - 用户消息：右对齐 / 不同颜色
  - 助手消息：左对齐，支持 Markdown 基础渲染（代码块高亮）
  - 工具调用：折叠显示（工具名 + 摘要）
- 流式输出：实时显示 assistant 正在生成的文本
- 键盘快捷键：Ctrl+C 退出、Esc 取消当前生成

**验收标准**:

- TUI 可启动，显示欢迎消息
- 输入文本 -> 发送到 API -> 流式显示回复
- 消息历史可滚动
- Ctrl+C 安全退出

**依赖**: 迭代 7

---

### 迭代 10：TUI 权限对话框 + Todo 面板

**目标**: 在 TUI 中集成权限确认和 Todo 显示。

**产出**:

- 权限确认对话框（模态弹窗）：
  - 显示工具名、参数摘要
  - 选项：Allow / Always Allow / Deny / Always Deny
  - 键盘操作：`y/n/a/d`
- Todo 侧面板：
  - 右侧折叠/展开面板
  - 显示当前 todo 列表（状态图标 + 内容）
  - 快捷键 Tab 切换面板
- 工具执行状态显示：
  - spinner 动画（工具执行中）
  - 结果折叠/展开

**验收标准**:

- 权限对话框：工具调用时弹出确认，选择后继续执行
- Always Allow 后同工具不再弹出
- Todo 面板：TodoWriteTool 调用后实时更新
- 工具执行有 spinner 反馈

**依赖**: 迭代 8, 迭代 9

---

### 迭代 11：System Prompt + 会话管理 + 斜杠命令

**目标**: 最终打磨，达到可日常使用的状态。

**产出**:

- System Prompt：
  - 参考 Claude Code 的 system prompt 结构
  - 包含工具使用说明、行为指导
  - 注入当前工作目录、OS 信息、日期等上下文
- 会话管理：
  - 会话持久化（`~/.config/rust-claude-code/sessions/`）
  - 继续上次会话 / 新建会话
- 斜杠命令：
  - `/clear` -> 清空当前会话
  - `/mode <mode>` -> 切换权限模式
  - `/todo` -> 显示/隐藏 todo 面板
  - `/help` -> 显示帮助
  - `/exit` -> 退出
- 命令行参数：
  - `--model` -> 指定模型
  - `--mode` -> 指定权限模式
  - `--continue` -> 继续上次会话
  - 直接传入 prompt（非交互模式）

**验收标准**:

- System prompt 包含完整的工具描述和行为指导
- 退出后重启可继续上次会话
- 所有斜杠命令功能正常
- 非交互模式：`rust-claude-code "explain this code"` 输出结果后退出

**依赖**: 迭代 10

---

## 8. 迭代依赖关系

```text
迭代 1 (类型系统)
├── 迭代 2 (API 非流式)
│   └── 迭代 3 (官方设计对齐)
│       └── 迭代 4 (SSE 流式)
│           └── 迭代 7 (查询循环) ──┐
├── 迭代 5 (Tool 框架 + Bash)       │
│   └── 迭代 6 (剩余工具)           │
│       └── 迭代 7 ─────────────────┤
│                                   ├── 迭代 8 (权限系统) ──┐
│                                   └── 迭代 9 (TUI 基础) ──┤
│                                                           └── 迭代 10 (TUI 权限 + Todo)
│                                                               └── 迭代 11 (最终打磨)
```

可并行开发的迭代对：

- 迭代 2 + 迭代 5（API 客户端与 Tool 系统互不依赖）
- 迭代 4 + 迭代 6（流式传输与工具实现互不依赖）
- 迭代 8 + 迭代 9（权限系统与 TUI 基础互不依赖）

其中，迭代 3 是设计收敛节点。它的主要职责是稳定 `core` 与 `api` 的公共边界，为迭代 4、7、8 减少返工。

---

## 9. 关键参考文件

| 文件 | 说明 | 行数 |
|------|------|------|
| `Tool.ts` | Tool 接口定义 | 792 |
| `tools.ts` | 工具注册表 | 389 |
| `Task.ts` | Task 类型定义 | 125 |
| `query.ts` | 核心查询循环 | 1729 |
| `claude.ts` | API 客户端 | 3419 |
| `toolOrchestration.ts` | 工具调度 | 188 |
| `BashTool/` | Bash 工具实现（18 文件） | - |
| `FileEditTool/FileEditTool.ts` | FileEdit 实现 | 625 |
| `FileReadTool/FileReadTool.ts` | FileRead 实现 | 1183 |
| `FileWriteTool/FileWriteTool.ts` | FileWrite 实现 | 434 |
| `TodoWriteTool/TodoWriteTool.ts` | TodoWrite 实现 | 115 |
| `EnterPlanModeTool/EnterPlanModeTool.ts` | Plan mode | 126 |
| `AppState.tsx` | 状态管理 | 200 |
| `permissions.ts` | 权限类型 | 441 |

所有参考源码位于：`/Users/yangchengxxyy/projects/claude-code-sourcemap/restored-src/src/`
