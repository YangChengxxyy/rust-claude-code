# Rust Claude Code — 需求与迭代计划

## 1. 项目目标

基于 Claude Code 的 TypeScript 源码（已通过 sourcemap 还原），开发一个 **Rust 版本**的 Claude Code，具备基础的**对话、编辑、规划**功能。

### 参考源码

- 路径: `/Users/yangchengxxyy/projects/claude-code-sourcemap/restored-src/src/`
- 核心文件：Tool.ts, tools.ts, Task.ts, query.ts, claude.ts, toolOrchestration.ts, AppState.tsx, permissions.ts
- 工具实现：BashTool, FileReadTool, FileEditTool, FileWriteTool, TodoWriteTool, EnterPlanModeTool

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

## 3. 源码分析发现

### 3.1 消息系统 (`types/message.js`)

- Message 分为 User / Assistant / System 三种角色
- ContentBlock 包含：Text, ToolUse, ToolResult, Thinking
- ToolUse 包含 id, name, input（JSON）
- ToolResult 包含 tool_use_id, content, is_error

### 3.2 Tool 系统 (`Tool.ts`, `tools.ts`)

- 每个工具实现统一的 Tool trait
- 核心方法：`call()`, `checkPermissions()`, `inputSchema`, `isReadOnly()`, `isConcurrencySafe()`
- 工具通过 `buildTool()` 工厂函数构建
- 工具注册表：按名称索引，支持动态查找

### 3.3 查询循环 (`query.ts`)

- 核心 agent 循环是一个 async generator
- 循环逻辑：发送消息 → 流式接收 → 检测 tool_use → 执行工具 → 拼接 tool_result → 继续循环
- 支持最大轮次限制
- tool_use 时自动拼接 tool_result 到消息历史

### 3.4 工具调度 (`services/tools/toolOrchestration.ts`)

- 工具分为并发安全和非并发安全两类
- 并发安全的工具（如 FileRead）可以并行执行
- 非并发安全的工具（如 Bash、FileEdit）串行执行
- 并行执行使用 Promise.all

### 3.5 权限系统 (`types/permissions.ts`)

- 5 种权限模式：
  - `default`: 默认模式，每次需要确认
  - `acceptEdits`: 自动接受文件编辑，其他需确认
  - `bypassPermissions`: 跳过所有权限检查
  - `plan`: 只读模式，不允许任何写操作
  - `dontAsk`: 不询问，拒绝需要确认的操作
- 支持 always_allow / always_deny 规则
- 规则可以附带 pattern（如 `Bash(git *)`）

### 3.6 核心工具实现

#### BashTool
- 功能丰富（18 个文件）
- 支持超时、工作目录设置
- 安全检查（危险命令检测）
- 输出截断

#### FileEditTool
- 基于 `old_string` → `new_string` 的查找替换模式
- 要求匹配唯一（除非 `replace_all=true`）
- 空 `old_string` + 非空 `new_string` = 创建新文件

#### FileReadTool
- 支持 offset / limit 分页读取
- 行号前缀输出
- 目录列表功能

#### FileWriteTool
- 创建/覆盖文件
- 自动创建父目录

#### TodoWriteTool
- 更新 AppState 中的 todo 列表
- 每个 todo 有 content, status (pending/in_progress/completed), priority (high/medium/low)
- 全部 completed 时清空列表

### 3.7 AppState

- 全局状态管理
- 包含：messages, todos, permission_context, current_mode 等
- React 上下文模式（Rust 中用 Arc<Mutex<>> 或 channel）

---

## 4. 项目结构设计

```
rust-claude-code/
├── Cargo.toml              # workspace 根配置
├── doc/
│   └── requirement.md      # 本文档
├── crates/
│   ├── core/               # 核心类型、消息系统、状态管理
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── message.rs       # Message, ContentBlock, Role
│   │       ├── tool_types.rs    # ToolUse, ToolResult, Tool trait
│   │       ├── permission.rs    # PermissionMode, PermissionRule
│   │       ├── state.rs         # AppState
│   │       └── config.rs        # 配置管理
│   ├── api/                # Anthropic API 客户端
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── client.rs        # API 客户端
│   │       ├── types.rs         # API 请求/响应类型
│   │       ├── stream.rs        # SSE 流式解析
│   │       └── error.rs         # API 错误类型
│   ├── tools/              # 工具实现
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── registry.rs      # ToolRegistry
│   │       ├── bash.rs          # BashTool
│   │       ├── file_read.rs     # FileReadTool
│   │       ├── file_edit.rs     # FileEditTool
│   │       ├── file_write.rs    # FileWriteTool
│   │       └── todo_write.rs    # TodoWriteTool
│   ├── tui/                # TUI 界面
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── app.rs           # TUI App 主循环
│   │       ├── chat_view.rs     # 聊天视图
│   │       ├── input.rs         # 输入框
│   │       ├── permission_dialog.rs  # 权限确认对话框
│   │       └── todo_panel.rs    # Todo 侧面板
│   └── cli/                # CLI 入口
│       ├── Cargo.toml
│       └── src/
│           └── main.rs          # 入口 + 参数解析
```

---

## 5. 核心类型设计（Rust）

### 5.1 Message 与 ContentBlock

```rust
pub enum Role {
    User,
    Assistant,
}

pub enum ContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String, input: serde_json::Value },
    ToolResult { tool_use_id: String, content: String, is_error: bool },
    Thinking { thinking: String },
}

pub struct Message {
    pub role: Role,
    pub content: Vec<ContentBlock>,
}
```

### 5.2 Tool Trait

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

```rust
pub enum PermissionMode {
    Default,
    AcceptEdits,
    BypassPermissions,
    Plan,
    DontAsk,
}

pub struct PermissionRule {
    pub tool_name: String,
    pub pattern: Option<String>,  // e.g. "git *"
}

pub enum PermissionCheck {
    Allowed,
    Denied { reason: String },
    NeedsConfirmation { prompt: String },
}
```

---

## 6. 迭代计划

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

**目标**: 实现基础的 Anthropic API 客户端，支持非流式 `create_message` 调用。

**产出**:
- `api` crate: AnthropicClient struct
- HTTP 请求构造（reqwest）
- API Key 认证
- 错误处理（APIError 类型，rate limit, auth error 等）
- 非流式 create_message 方法

**验收标准**:
- 集成测试：能用真实 API Key 发送一条消息并收到回复
- 单元测试：请求序列化正确，错误类型覆盖完整
- 示例程序（`examples/simple_chat.rs`）：终端交互式对话

**依赖**: 迭代 1

---

### 迭代 3：SSE 流式传输

**目标**: 支持 Anthropic API 的 SSE 流式响应。

**产出**:
- `api` crate: SSE 事件解析器
- Delta 累积器：将 content_block_delta 事件累积为完整的 ContentBlock
- MessageStream：返回 `Stream<Item = StreamEvent>` 的异步流
- StreamEvent 枚举：MessageStart, ContentBlockStart, ContentBlockDelta, ContentBlockStop, MessageDelta, MessageStop

**验收标准**:
- 单元测试：SSE 文本解析正确（event, data 字段提取）
- 单元测试：Delta 累积正确（多个 text delta → 完整 text）
- 集成测试：流式接收一条完整消息，逐 token 打印
- 示例程序（`examples/streaming_chat.rs`）：实时打印 token

**依赖**: 迭代 2

---

### 迭代 4：Tool 系统框架 + BashTool

**目标**: 实现 Tool trait、ToolRegistry，并完成第一个工具 BashTool。

**产出**:
- `tools` crate: ToolRegistry（按名称注册/查找工具）
- `tools` crate: BashTool 实现
  - 执行 shell 命令（tokio::process::Command）
  - 支持 timeout 参数
  - 支持 workdir 参数
  - 输出截断（超长输出时截取首尾）
  - 危险命令检测（rm -rf /, sudo 等）
- 工具输入验证（基于 input_schema）

**验收标准**:
- 单元测试：BashTool 执行简单命令（echo, ls）返回正确输出
- 单元测试：超时功能（执行 sleep 命令验证超时）
- 单元测试：工作目录切换
- 单元测试：危险命令检测
- 单元测试：ToolRegistry 注册/查找/列出工具

**依赖**: 迭代 1

---

### 迭代 5：FileRead + FileEdit + FileWrite + TodoWrite

**目标**: 实现剩余 4 个核心工具。

**产出**:
- `tools` crate: FileReadTool
  - 读取文件内容，支持 offset/limit
  - 行号前缀
  - 目录列表模式
- `tools` crate: FileEditTool
  - old_string → new_string 替换
  - 唯一匹配检查
  - replace_all 选项
  - 创建新文件（空 old_string）
- `tools` crate: FileWriteTool
  - 创建/覆盖文件
  - 自动创建父目录
- `tools` crate: TodoWriteTool
  - 更新 AppState 中的 todo 列表
  - 状态管理（pending/in_progress/completed）
  - 全部完成时清空

**验收标准**:
- FileReadTool：读取已知文件，验证行号和内容正确；分页读取；目录列出
- FileEditTool：单次替换、多次替换（replace_all）、唯一性检查失败、创建新文件
- FileWriteTool：写入新文件、覆盖已有文件、自动创建目录
- TodoWriteTool：添加/更新/清空 todo，状态转换
- 所有工具注册到 ToolRegistry

**依赖**: 迭代 4

---

### 迭代 6：查询循环（Query Loop）

**目标**: 实现核心 agent 循环，串联 API 调用和工具执行。

**产出**:
- `core` crate: QueryLoop struct
  - 发送消息到 API
  - 流式接收响应
  - 检测 tool_use blocks
  - 执行工具（尊重并发安全性）
  - 拼接 tool_result 到消息历史
  - 继续循环直到 stop_reason != tool_use
- 工具调度逻辑
  - 并发安全工具并行执行（tokio::join!）
  - 非并发安全工具串行执行
- 最大轮次限制
- CLI 入口程序（`cli` crate main.rs）：stdin 读取用户输入，调用 QueryLoop

**验收标准**:
- 集成测试：发送包含 tool_use 的对话，验证工具被正确调用
- 集成测试：多轮工具调用（API 连续返回 tool_use）
- 集成测试：并发工具执行（多个 FileRead 并行）
- CLI 可运行：`cargo run -- "list files in current directory"` 正确调用 BashTool

**依赖**: 迭代 3, 迭代 5

---

### 迭代 7：权限系统

**目标**: 实现完整的 5 种权限模式。

**产出**:
- `core` crate: PermissionManager
  - 权限检查逻辑
  - always_allow / always_deny 规则管理
  - 规则持久化（~/.config/rust-claude-code/permissions.json）
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

**依赖**: 迭代 6

---

### 迭代 8：TUI 基础框架

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
- 输入文本 → 发送到 API → 流式显示回复
- 消息历史可滚动
- Ctrl+C 安全退出

**依赖**: 迭代 6

---

### 迭代 9：TUI 权限对话框 + Todo 面板

**目标**: 在 TUI 中集成权限确认和 Todo 显示。

**产出**:
- 权限确认对话框（模态弹窗）：
  - 显示工具名、参数摘要
  - 选项：Allow / Always Allow / Deny / Always Deny
  - 键盘操作：y/n/a/d
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

**依赖**: 迭代 7, 迭代 8

---

### 迭代 10：System Prompt + 会话管理 + 斜杠命令

**目标**: 最终打磨，达到可日常使用的状态。

**产出**:
- System Prompt：
  - 参考 Claude Code 的 system prompt 结构
  - 包含工具使用说明、行为指导
  - 注入当前工作目录、OS 信息、日期等上下文
- 会话管理：
  - 会话持久化（~/.config/rust-claude-code/sessions/）
  - 继续上次会话 / 新建会话
- 斜杠命令：
  - `/clear` — 清空当前会话
  - `/mode <mode>` — 切换权限模式
  - `/todo` — 显示/隐藏 todo 面板
  - `/help` — 显示帮助
  - `/exit` — 退出
- 命令行参数：
  - `--model` — 指定模型
  - `--mode` — 指定权限模式
  - `--continue` — 继续上次会话
  - 直接传入 prompt（非交互模式）

**验收标准**:
- System prompt 包含完整的工具描述和行为指导
- 退出后重启可继续上次会话
- 所有斜杠命令功能正常
- 非交互模式：`rust-claude-code "explain this code"` 输出结果后退出

**依赖**: 迭代 9

---

## 7. 迭代依赖关系

```
迭代 1 (类型系统)
├── 迭代 2 (API 非流式)
│   └── 迭代 3 (SSE 流式)
│       └── 迭代 6 (查询循环) ──┐
├── 迭代 4 (Tool框架 + Bash)    │
│   └── 迭代 5 (剩余工具)       │
│       └── 迭代 6 ─────────────┤
│                               ├── 迭代 7 (权限系统) ──┐
│                               └── 迭代 8 (TUI 基础) ──┤
│                                                       └── 迭代 9 (TUI 权限+Todo)
│                                                           └── 迭代 10 (最终打磨)
```

可并行开发的迭代对：
- 迭代 2 + 迭代 4（API 客户端 与 Tool 系统互不依赖）
- 迭代 3 + 迭代 5（流式传输 与 工具实现互不依赖）
- 迭代 7 + 迭代 8（权限系统 与 TUI 基础互不依赖）

---

## 8. 关键参考文件

| 文件 | 说明 | 行数 |
|------|------|------|
| `Tool.ts` | Tool trait 定义 | 792 |
| `tools.ts` | 工具注册表 | 389 |
| `Task.ts` | Task 类型定义 | 125 |
| `query.ts` | 核心查询循环 | 1729 |
| `claude.ts` | API 客户端 | 3419 |
| `toolOrchestration.ts` | 工具调度 | 188 |
| `BashTool/` | Bash 工具（18 文件） | — |
| `FileEditTool/FileEditTool.ts` | FileEdit 实现 | 625 |
| `FileReadTool/FileReadTool.ts` | FileRead 实现 | 1183 |
| `FileWriteTool/FileWriteTool.ts` | FileWrite 实现 | 434 |
| `TodoWriteTool/TodoWriteTool.ts` | TodoWrite 实现 | 115 |
| `EnterPlanModeTool/EnterPlanModeTool.ts` | Plan mode | 126 |
| `AppState.tsx` | 状态管理 | 200 |
| `permissions.ts` | 权限类型 | 441 |

所有参考源码位于：`/Users/yangchengxxyy/projects/claude-code-sourcemap/restored-src/src/`
