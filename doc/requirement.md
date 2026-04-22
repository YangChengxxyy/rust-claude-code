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
| 工具范围 | Core 7 | Bash, FileRead, FileEdit, FileWrite, TodoWrite, Glob, Grep |
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
- `ContentBlock::Thinking` 支持 `signature: Option<String>` 字段，兼容 extended thinking 多轮对话回传。
- `ContentBlock::Unknown` 带 `#[serde(other)]`，forward-compatible 处理未知 API 响应块类型。
- 已实现 `ToolResult`、`ToolResultMetadata`、`ToolInfo`。
- 已实现 `PermissionMode`、`PermissionRule`、`PermissionCheck`，并收敛了统一权限检查入口与规则优先级边界。
- 已实现 `AppState`、`SessionSettings`、`TodoItem`、`TodoStatus`、`TodoPriority`。
- `SessionSettings` 已包含 `thinking_enabled: bool`（默认 true）和 `stream` 字段。
- 已实现 `Config`，支持从配置文件或 `ANTHROPIC_API_KEY` 加载 API Key，支持 `base_url`、`bearer_auth` 与 `ANTHROPIC_AUTH_TOKEN` 自动检测。支持 `ConfigProvenance` 追踪每个配置项来源。
- 已实现 `ClaudeSettings`（`settings.rs`），读取 `~/.claude/settings.json` 的 `env`、`model`、`apiKeyHelper`、`permissions` 字段。支持项目级 `.claude/settings.json` 发现与 user/project 层合并。
- 已实现 `Model`（`model.rs`），支持模型名称标准化、`ThinkingConfig`（Disabled/Enabled/Adaptive）、`get_thinking_config_for_model()` 自动选择、`usage_exceeds_200k_tokens()` 阈值检查。
- 已实现 `claude_md` 模块（`claude_md.rs`），支持 CLAUDE.md 发现（全局 + 项目级）、git root 检测、内容截断。
- 已实现 `compaction` 模块（`compaction.rs`），支持 usage-based token 估算（优先 API usage 数据，chars/4 兜底）、消息分区、`needs_compaction` 检查。
- 已实现 `git` 模块（`git.rs`），支持 `GitContextSnapshot` 收集（branch、clean 状态、最近 commits）。

### 4.2 api crate

- 已实现非流式 `AnthropicClient`。
- 已支持基于 `reqwest` 的 `POST /v1/messages` 调用。
- 已支持 `x-api-key` 和 `anthropic-version` 请求头。
- 已实现基础错误映射：认证失败、限流、通用 API 错误、超时、连接错误。
- 已实现 `CreateMessageRequest`、`CreateMessageResponse`、`ApiMessage`、`ApiTool`、`SystemPrompt`。
- `CreateMessageRequest` 已支持 `metadata` 和 `thinking` 字段（`ThinkingConfig` 序列化）。
- `SystemPrompt` 支持结构化 `SystemBlock` 块，带 `cache_control: { type: "ephemeral" }` 标记。
- 已实现 `inject_cache_control_on_messages()` — 在最后一条消息的最后一个内容块上注入 `cache_control`。
- 已实现 SSE 流式基础设施：`MessageStream`、`StreamEvent`、SSE 事件解析、真实流式请求入口与 `examples/streaming_chat.rs`。
- 已实现 delta 累积器，支持 `text_delta`、`thinking_delta`、`signature_delta` 与 `input_json_delta` 还原完整内容块。
- 已新增 `examples/simple_chat.rs` 与真实 API 的忽略型集成测试。

### 4.3 tools crate

- 已实现 `Tool` trait、可执行的 `ToolRegistry` 与 7 个工具。
- `BashTool` 已支持 shell 执行、timeout、workdir、危险命令检测与输出截断。
- 已实现 `FileReadTool`、`FileEditTool`、`FileWriteTool`、`TodoWriteTool`，并完成工具注册与基础测试覆盖。
- 已实现 `GlobTool`（`glob.rs`），支持 glob pattern 文件搜索，按修改时间排序，标记为只读且并发安全。
- 已实现 `GrepTool`（`grep.rs`），支持 regex 内容搜索、`-A`/`-B`/`-C` 上下文行、glob/type 过滤、大小写不敏感、`head_limit`，标记为只读且并发安全。

### 4.4 cli crate

- 已实现最小可运行的 CLI 入口，支持配置加载、`AppState` 初始化与单 prompt 非交互模式。
- 已接入 `QueryLoop`、流式响应消费、工具执行与多轮 tool use。
- 已支持 `--mode` / `-m` 参数切换权限模式（`default`、`accept-edits`、`bypass`、`plan`、`dont-ask`）。
- 无 prompt 启动时，已可进入基础 TUI 交互模式，并通过现有 `QueryLoop` 执行用户输入。
- 已实现 Claude Code 兼容的 CLI 参数体系：`--model`、`-p/--print`、`--output-format`、`--max-turns`、`--system-prompt`/`--system-prompt-file`、`--append-system-prompt`/`--append-system-prompt-file`、`--max-tokens`、`--no-stream`、`--thinking`/`--no-thinking`、`--verbose`、`--allowed-tools`、`--disallowed-tools`、`--settings`。
- 已实现统一优先级链：`RUST_CLAUDE_*` 环境变量 > CLI 参数 > `ANTHROPIC_*` 环境变量（含 `~/.claude/settings.json` env 注入）> project settings > user settings > 配置文件 > 默认值。
- 已支持通过 `--settings` 指定自定义 settings.json 路径，默认读取 `~/.claude/settings.json` 的 `env` 字段注入进程环境变量（不覆盖已有值）。
- 已支持项目级 `.claude/settings.json` 自动发现与 user/project 层 permissions 合并。
- 已支持工具白名单/黑名单过滤（`--allowed-tools`、`--disallowed-tools`）。
- 已支持 `--output-format json` 以 JSON 格式输出最终消息。
- 已实现 System Prompt 组合模块（`system_prompt.rs`）：核心行为指导、动态工具描述注入、环境上下文、Git 上下文注入、CLAUDE.md 内容注入、自定义追加。
- QueryLoop 构建请求时自动注入 thinking 配置（根据模型选择 adaptive/budget/disabled）和 prompt caching（system prompt + 消息级 `cache_control`）。
- QueryLoop 已实现 Max Tokens Recovery：`stop_reason == MaxTokens` 时自动注入恢复消息继续生成（最多 3 次）。
- 已实现会话持久化（`session.rs`）：JSON 格式保存/加载，支持 `--continue` / `-c` 恢复最近会话。
- 已实现 Compaction 服务（`compaction.rs`）：LLM 摘要压缩、可配置阈值、手动 `/compact` 与 QueryLoop 自动压缩集成。
- 已实现 Hooks 系统（`hooks.rs`）：`HookRunner` 支持 command 类型 hook、matcher 过滤、JSON stdin/stdout 通信、超时保护、exit code 语义。QueryLoop 集成 PreToolUse/PostToolUse/UserPromptSubmit/Stop 四个事件触发点。

### 4.5 tui crate

- 已实现基础 TUI 框架：`App` 状态对象、`ChatMessage` / `AppEvent` 事件模型（35+ 事件变体）、渲染层与终端守卫。
- 已实现顶部状态栏（模型、模式、git branch、token 用量）、中间聊天区域、底部多行输入框的基础布局与样式（`theme.rs`）。
- 已实现完整键盘交互：Enter 发送、Shift+Enter 换行、Ctrl+C 退出/取消、Esc 清空/取消流式、左右移动光标、Ctrl+Left/Ctrl+Right 按词移动、Home/End/Ctrl+A/Ctrl+E 行内导航、Up/Down 历史浏览/多行移动、PageUp/PageDown 聊天区域滚动、Ctrl+Home/Ctrl+End 跳转聊天区域边界、Ctrl+L 清屏重绘、Tab 切换 thinking block 展开/折叠、Backspace/Delete。
- 已实现多行输入 `InputBuffer`，支持 Bracketed Paste 粘贴多行内容。
- 已实现输入历史持久化（500 条上限，文件存储），Up/Down 浏览，浏览时暂存当前草稿。
- 已实现 Markdown 基础渲染：标题（H1-H3 颜色区分）、有序/无序列表、代码块（带语言标签和边框）、段落、行内代码/粗体/斜体解析。
- 已实现 TUI 事件桥接 `TuiBridge`，支持流式文本（token 级）、工具调用、工具结果、usage 更新、thinking 阶段（流式 delta + 折叠/展开）、压缩事件、状态更新、配置信息等。
- 已实现权限确认对话框（模态弹窗）：居中弹窗显示工具名与参数摘要，支持 Allow(y)/AlwaysAllow(a)/Deny(n)/AlwaysDeny(d) 四选项，方向键导航 + Enter 确认。
- 已实现 Todo 侧面板（Tab 键切换）：右侧 30 列面板，状态图标（○/◐/●），实时刷新。
- 已实现斜杠命令：`/clear`、`/compact`、`/config`、`/cost`、`/diff`、`/hooks`、`/mode`、`/model`、`/todo`、`/help`、`/exit`。
- QueryLoop 中 `NeedsConfirmation` 已通过 TuiBridge oneshot channel 接入交互式权限对话框。
- 已实现 Hook 阻止事件显示（`HookBlocked` 事件 → 系统消息）。

---

## 5. 设计边界说明

本节记录当前已知的设计边界，避免把初版设想误当作当前定稿。

### 5.1 Message 与 ContentBlock

当前 Rust 版本采用 `role + content blocks` 的消息模型，已覆盖文本、工具调用、工具结果与 thinking block。

已知待收敛点：

- `system` 内容当前主要通过 API 请求层的 `system` 字段承载，而不是 `Role::System`。
- `ContentBlock::ToolResult` 与 `tool_types::ToolResult` 的建模边界仍需在迭代 3 中进一步对齐。

### 5.2 Tool 接口

参考设计中的 Tool 接口已在 `tools` crate 中正式落地（`tool.rs`），通过 `Tool` trait 统一了 7 个工具的接口：

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn info(&self) -> ToolInfo;         // 名称、描述、input_schema
    fn is_read_only(&self) -> bool;
    fn is_concurrency_safe(&self) -> bool;
    async fn execute(&self, input: serde_json::Value, context: ToolContext) -> Result<ToolResult, ToolError>;
}
```

权限检查已收敛到 `PermissionManager` 统一处理，不在 Tool trait 内。

### 5.3 Permission 系统

当前 Rust 版本已经实现：

- `PermissionManager`
- always_allow / always_deny 规则管理与 JSON 持久化
- Query Loop 集成权限检查
- CLI `--mode` 参数切换权限模式
- TUI 交互式权限确认对话框（模态弹窗，支持 Allow/AlwaysAllow/Deny/AlwaysDeny）
- 通过 `TuiBridge` oneshot channel 实现 QueryLoop 与 TUI 权限对话框的双向通信

### 5.4 AppState

当前 `AppState` 已包含以下字段：

- `messages`
- `todos`
- `permission_mode`
- `model`
- `max_tokens`
- `cwd`
- `total_usage`

已在 `SessionSettings` 中扩展会话配置。权限上下文已通过 `PermissionManager` 统一收敛。

---

## 6. 项目结构设计

```text
rust-claude-code/
├── Cargo.toml              # workspace 根配置
├── CLAUDE.md               # 项目指令文件
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
│   │       ├── config.rs
│   │       ├── settings.rs      # ClaudeSettings（~/.claude/settings.json）
│   │       ├── model.rs         # 模型名称标准化与用量阈值
│   │       ├── claude_md.rs     # CLAUDE.md 发现与加载
│   │       ├── compaction.rs    # token 估算与消息分区
│   │       └── hooks.rs         # Hook 事件、配置、输入/结果类型
│   ├── api/                # Anthropic API 客户端
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── client.rs
│   │       ├── types.rs
│   │       ├── streaming.rs     # SSE 解析与 delta 累积器
│   │       └── error.rs
│   ├── tools/              # 工具实现（7 个工具）
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── tool.rs          # Tool trait 与 ToolContext
│   │       ├── registry.rs
│   │       ├── bash.rs
│   │       ├── file_read.rs
│   │       ├── file_edit.rs
│   │       ├── file_write.rs
│   │       ├── todo_write.rs
│   │       ├── glob.rs          # GlobTool（文件搜索）
│   │       └── grep.rs          # GrepTool（内容搜索）
│   ├── tui/                # TUI 界面
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs           # TerminalGuard
│   │       ├── app.rs           # App 状态、事件循环、权限对话框、斜杠命令
│   │       ├── ui.rs            # 渲染层（聊天区、状态栏、输入框、Todo 面板）
│   │       ├── events.rs        # AppEvent、ChatMessage、PermissionResponse
│   │       ├── bridge.rs        # TuiBridge（QueryLoop → TUI 桥接）
│   │       └── theme.rs         # 主题常量与样式
│   └── cli/                # CLI 入口
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs
│           ├── query_loop.rs    # ModelClient trait、QueryLoop 代理循环
│           ├── system_prompt.rs # System prompt 组合
│           ├── session.rs       # 会话持久化
│           ├── compaction.rs    # CompactionService（LLM 摘要压缩）
│           ├── hooks.rs         # HookRunner（hook 执行引擎）
│           └── lib.rs
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

**状态**: 已完成

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

**状态**: 已完成

**完成记录**:

- 已实现 `core` crate 的 `PermissionManager`
- 已支持规则持久化到 `~/.config/rust-claude-code/permissions.json`
- 已支持紧凑规则格式解析与序列化：`Bash`、`Bash(git *)`、`FileRead`
- 已将权限检查集成到 `QueryLoop` 的工具执行前置路径
- 已为 `Bash` 工具提取 `command` 字段参与 pattern 匹配
- 对 `NeedsConfirmation` 场景已在当前 CLI 路径中暂时按拒绝处理，并返回明确错误信息
- CLI 已支持 `--mode` / `-m` 参数切换权限模式
- 已补充规则解析、持久化、模式行为与 QueryLoop 权限集成测试
- 验证结果：`cargo test --workspace` 通过

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

**状态**: 已完成

**完成记录**:

- 已实现 `tui` crate 的基础模块：`app.rs`、`ui.rs`、`events.rs`、`bridge.rs`
- 已实现 `App` 状态对象与 `ChatMessage` / `AppEvent` 事件模型
- 已实现顶部状态栏、中间聊天区域、底部输入框的基础布局与样式
- 已实现基础键盘交互：Enter、Ctrl+C、Esc、方向键、Home / End、Backspace / Delete
- 已实现流式文本显示状态与结束后落盘为 assistant 消息的状态切换
- 已实现 `TuiBridge`，支持流式文本、工具调用、工具结果、usage 和错误事件
- 已实现终端初始化/清理守卫 `TerminalGuard`，处理 raw mode、alternate screen、mouse capture 与 panic 清理
- 已补充 TUI 单元测试覆盖状态迁移、事件传递与基础渲染辅助逻辑
- 验证结果：`cargo check -p rust-claude-tui` 通过，`cargo test -p rust-claude-tui` 通过，`cargo test --workspace` 通过

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
- 键盘快捷键：Ctrl+C 退出、Esc 取消当前生成、PageUp/PageDown 滚动聊天区域、Ctrl+Home/Ctrl+End 跳转到最旧/最新聊天内容

**验收标准**:

- TUI 可启动，显示欢迎消息
- 输入文本 -> 发送到 API -> 流式显示回复
- 消息历史可滚动
- Ctrl+C 安全退出

**依赖**: 迭代 7

---

### 迭代 10：TUI 权限对话框 + Todo 面板

**状态**: 已完成

**完成记录**:

- 已实现权限确认对话框（模态弹窗）：
  - 居中弹窗显示工具名、参数摘要
  - 支持 4 个选项：Allow(y) / Always Allow(a) / Deny(n) / Always Deny(d)
  - 上下方向键导航，Enter 确认当前选项
  - Always Allow/Deny 自动添加到权限规则列表
- 已实现 TuiBridge 权限请求通道（oneshot channel 双向通信）
- 已将 QueryLoop 中的 `NeedsConfirmation` 硬编码拒绝替换为交互式权限对话框
- 已实现 Todo 侧面板（Tab 键切换显示/隐藏）：
  - 右侧 30 列面板，显示 TodoItem 列表
  - 状态图标：○ pending / ◐ in_progress / ● completed
  - TodoUpdate 事件支持实时刷新
- 已实现 QueryLoop 到 TUI 的完整事件桥接：
  - StreamDelta 实时推送到 TUI（token 级流式显示）
  - ThinkingStart 事件推送（thinking 阶段显示）
  - ToolUseStart / ToolResult 事件推送
  - StreamEnd / UsageUpdate 事件推送
- 已补充权限对话框、Todo 面板、桥接通道的单元测试
- 验证结果：`cargo test --workspace` 通过

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

**状态**: 已完成

**完成记录**:

- 已实现 System Prompt 组合模块（`cli/src/system_prompt.rs`）：
  - 核心行为指导、工具使用说明
  - 动态注入可用工具描述列表
  - 注入 CWD、OS、架构、日期等环境上下文
  - 支持 `--append-system-prompt` 自定义追加
  - 当无显式 `--system-prompt` 时自动使用组合 prompt
- 已实现会话持久化（`cli/src/session.rs`）：
  - JSON 格式保存到 `~/.config/rust-claude-code/sessions/`
  - 每次查询结束后自动保存
  - 支持 `--continue` / `-c` 参数恢复最近会话
  - SessionFile 包含 id、model、cwd、timestamps、messages
- 已实现斜杠命令：
  - `/clear` — 清空当前会话消息
  - `/mode <mode>` — 切换权限模式（支持 5 种模式）
  - `/todo` — 切换 Todo 面板显示
  - `/help` — 显示可用命令帮助
  - `/exit` — 退出程序
  - 未知命令显示友好错误提示
- 已补充 system prompt、session、slash command 的单元测试
- 验证结果：`cargo test --workspace` 通过，218 个测试全部通过

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

---

## 10. 第二期项目规划（与原版 Claude Code 的功能对齐）

### 10.1 背景

当前 Rust 版本已经完成第一期目标，具备基础可用的对话、工具调用、权限管理、TUI 交互、system prompt、会话持久化与 slash command 能力。

对照原版 Claude Code TypeScript 实现后，当前差距已经不再集中在“能否运行”，而是集中在“是否具备完整工程化能力与生态能力”。因此，第二期项目的目标不再是补齐基础骨架，而是围绕高频实用能力、长会话能力、扩展能力和工程协作能力，逐步把 Rust 版本推进到更接近原版 Claude Code 的可日常重度使用状态。

第二期的规划依据：

- 原版参考源码：`/Users/yangchengxxyy/projects/claude-code-sourcemap/restored-src/src/`
- 当前 Rust 实现：本仓库 `crates/core`、`crates/api`、`crates/tools`、`crates/cli`、`crates/tui`
- 差距分析结论：优先补齐高频工具、项目指令系统、上下文压缩、Git 集成、TUI 增强，再逐步推进 hooks、MCP、memory、agent/task/team 等高级能力

### 10.2 第二期总体目标

第二期聚焦以下四类能力：

1. **高频基础能力补齐**
   - 增加文件搜索、内容搜索、代码导航等高频工具
   - 增强 slash command、配置系统与 Git 集成
   - 提升 TUI 的可读性和交互效率

2. **长会话与项目上下文能力**
   - 支持 `CLAUDE.md` 项目指令加载
   - 引入上下文压缩（compaction）能力
   - 为后续 memory / team memory 预留边界

3. **扩展与自动化能力**
   - 引入 hooks 系统
   - 支持 MCP 客户端与工具桥接
   - 为 skills / plugin / remote bridge 预留接口

4. **多代理与任务编排能力**
   - 引入 task / agent / team 基础抽象
   - 支持更复杂的 agentic workflow

### 10.3 第二期范围分层

#### P0：优先落地（高频、刚需、收益最大）

- ~~`GlobTool`~~ ✅ 已完成（迭代 12）
- ~~`GrepTool`~~ ✅ 已完成（迭代 12）
- ~~`CLAUDE.md` 加载与 system prompt 注入~~ ✅ 已完成（迭代 13）
- ~~上下文压缩（至少先实现基础 compact）~~ ✅ 已完成（迭代 14）
- Git 基础集成（git root、branch、worktree 感知）
- TUI Markdown 基础渲染与交互增强
- 新增高频 slash commands：~~`/compact`~~ ✅、~~`/model`~~ ✅、`/cost`、`/usage`、`/diff`、`/config`

#### P1：第二批落地（显著增强工程能力）

- `LSPTool`
- hooks 系统
- 项目级 `.claude/settings.json`
- 配置来源合并与验证增强
- 工具大输出落盘 / 缓存策略
- 文件状态缓存
- 更多 slash commands：`/memory`、`/permissions`、`/hooks`、`/session`、`/init`

#### P2：高级能力（接近原版完整生态）

- MCP 客户端与 MCP 工具桥接
- memory directory 系统（`MEMORY.md`、typed memories）
- `AgentTool` 与基础 task 系统
- team / send-message / task orchestration
- skills / plugin 机制
- 远程桥接、coordinator、assistant mode 等高级能力

### 10.4 第二期不在首批范围内的内容

以下内容记录为长期方向，但不作为第二期前半段的强制交付项：

- 完整插件市场与插件安装生态
- mobile / desktop / remote bridge 全量体验
- auto dream、advisor、assistant perpetual mode
- 企业级策略配置（managed settings / MDM / policy settings）
- 原版中大量内部命令与实验性命令的 1:1 复刻

原则上，第二期优先追求“高频可用 + 架构可扩展”，不追求一次性完整复制原版所有边缘能力。

### 10.5 第二期迭代计划

### 迭代 12：搜索工具补齐（Glob + Grep）

**状态**: 已完成

**完成记录**:

- 已实现 `tools` crate 的 `GlobTool`（`glob.rs`）：
  - 支持 glob pattern 文件搜索（`**/*.rs`、`*.toml` 等）
  - 支持 `path` 参数指定搜索根目录，默认使用 CWD
  - 返回按修改时间排序（最近优先）的匹配结果
  - 标记为 `is_read_only = true`、`is_concurrency_safe = true`
- 已实现 `tools` crate 的 `GrepTool`（`grep.rs`）：
  - 支持 regex 内容搜索（使用 `regex` crate）
  - 支持 `files_with_matches`（默认）和 `content` 两种输出模式
  - 支持 `-A`/`-B`/`-C` 上下文行参数
  - 支持 `glob` 文件名过滤和 `type` 文件类型过滤（覆盖 30+ 种常见语言）
  - 支持 `-i` 大小写不敏感搜索
  - 支持 `head_limit` 结果限制（默认 250）
  - 使用 `walkdir` 遍历目录，自动跳过隐藏目录和二进制文件
  - 标记为 `is_read_only = true`、`is_concurrency_safe = true`
- 已在 `cli/src/main.rs` 注册两个工具到 `ToolRegistry`
- 已更新 `cli/src/system_prompt.rs` 的核心 prompt 增加搜索工具使用指引
- 新增依赖：`glob 0.3`、`walkdir 2`、`regex 1`
- 已补充完整的单元测试覆盖（GlobTool 5 个 + GrepTool 11 个 + 辅助函数 3 个）
- 验证结果：`cargo test --workspace` 通过，233 个测试全部通过

**目标**: 补齐最常用的文件搜索与内容搜索工具，显著提升代码库探索效率。

**产出**:

- `tools` crate: `GlobTool`
  - 支持 glob pattern 文件搜索
  - 支持相对 / 绝对路径起点
  - 返回排序后的匹配结果
- `tools` crate: `GrepTool`
  - 支持基于 regex 的内容搜索
  - 支持 path、glob、type、context、head_limit 等常用参数
  - 支持仅返回文件名 / 返回匹配内容两种模式
- 工具注册与 QueryLoop 集成
- system prompt 中增加工具说明

**验收标准**:

- `GlobTool` 能在中大型代码库中正确返回匹配文件
- `GrepTool` 能正确搜索内容并支持上下文输出
- 与现有权限系统、工具注册表、QueryLoop 正常协作
- `cargo test --workspace` 通过

**依赖**: 迭代 11

---

### 迭代 13：项目指令系统（CLAUDE.md）

**状态**: 已完成

**完成记录**:

- 已实现 `core` crate 的 `claude_md` 模块（`claude_md.rs`）：
  - 支持发现全局（`~/.claude/CLAUDE.md`）与项目级 `CLAUDE.md`
  - 通过 git root 检测确定项目根目录
  - 支持从 CWD 向上遍历目录查找 `CLAUDE.md`
  - 叶目录优先（leaf-most priority）的合并策略
  - 内容截断（默认 30K 字符），防止 system prompt 过大
- 已在 `cli/src/system_prompt.rs` 中集成 CLAUDE.md 注入：
  - 组合顺序：核心行为 < 工具描述 < 环境上下文 < CLAUDE.md < 自定义追加
  - 当无显式 `--system-prompt` 时自动发现并注入
- 已在 `cli/src/main.rs` 中接入发现流程
- 验证结果：`cargo test --workspace` 通过

**目标**: 让 Rust 版本支持项目级协作指令，与原版 Claude Code 的项目上下文机制对齐。

**产出**:

- `cli` / `core`：支持发现并读取当前目录及父目录中的 `CLAUDE.md`
- 支持用户级全局指令文件（路径后续按实现方案确定）
- 将项目指令注入 system prompt 构建流程
- 明确多份指令文件的合并顺序、截断规则与去重边界

**验收标准**:

- 在有 `CLAUDE.md` 的项目中，system prompt 能稳定包含项目指令
- 父目录查找逻辑正确，避免重复加载
- 指令内容不会破坏现有 system prompt 结构
- 相关单元测试和集成测试通过

**依赖**: 迭代 11

---

### 迭代 14：长会话能力（基础 Compaction）

**状态**: 已完成

**完成记录**:

- 已实现 `core` crate 的 `compaction` 模块（`compaction.rs`）：
  - token 估算（char_count / 4 启发式）
  - 消息分区（保留最近上下文 + 提取待压缩历史）
  - `needs_compaction` 阈值检查
  - 可配置的 `CompactionConfig`（context window、threshold ratio、preserve ratio、summary max tokens）
- 已实现 `cli` crate 的 `CompactionService`（`compaction.rs`）：
  - 基于 LLM 的摘要生成（通过 `ModelClient` 调用 API）
  - 支持手动 `/compact` 斜杠命令
  - 支持 QueryLoop 每轮自动压缩（`auto_compact_if_needed`）
  - 压缩后保留必要对话语义与最近上下文
- 已在 TUI 中集成 `CompactionStart` / `CompactionComplete` 事件
- 已补充 compaction 单元测试（threshold 判定、成功压缩、API 失败、消息不足等场景）
- 验证结果：`cargo test --workspace` 通过

**目标**: 解决长对话上下文不断膨胀的问题，为日常重度使用提供基础保障。

**产出**:

- `cli` 内新增 compact 模块，`core` 内新增 compaction 基础设施
- 支持手动 `/compact` 命令
- 支持在达到 token 阈值时触发基础压缩
- 压缩后保留必要对话语义、最近上下文和工具结果摘要
- 为后续 micro-compact / session memory compact 预留边界

**验收标准**:

- 长对话场景下可手动触发 compact 并继续正常对话
- 压缩后消息历史结构仍可被 QueryLoop 正常消费
- 不破坏现有会话持久化与 TUI 展示逻辑
- `cargo test --workspace` 通过

**依赖**: 迭代 13

---

### 迭代 15：API 能力增强（Prompt Caching + Extended Thinking）

**状态**: 已完成

**完成记录**:

- 已实现 `api` crate Prompt Caching 基础设施：
  - `SystemBlock` 支持 `cache_control` 字段（`with_cache_control()` 方法）
  - `inject_cache_control_on_messages()` — 在最后一条消息的最后一个内容块上注入 `cache_control: { type: "ephemeral" }`
  - QueryLoop 构建请求时自动注入 system prompt 和消息级 cache_control 标记
- 已实现 Extended Thinking 全链路支持：
  - `ThinkingConfig` 枚举：`Disabled` / `Enabled { budget_tokens }` / `Adaptive`
  - `get_thinking_config_for_model()` — Opus/Sonnet 4.6 使用 Adaptive，其他模型使用 budget，不支持 thinking 的模型自动 Disabled
  - `SessionSettings.thinking_enabled: bool`（默认 true）
  - `ContentBlock::Thinking` 增加 `signature: Option<String>` 字段
  - 流式累积器支持 `SignatureDelta` 事件，正确还原 thinking block 的 signature
  - `--thinking` / `--no-thinking` CLI 参数
- 已实现 Max Tokens Recovery：
  - 当 `stop_reason == MaxTokens` 时自动注入恢复消息继续生成（最多 3 次）
  - 截断时如有 tool_use blocks 会先执行再续写
- 已实现 Forward-compatible 响应解析：
  - `ContentBlock::Unknown` 带 `#[serde(other)]`，跳过 `server_tool_use` 等未知块类型
- 已实现 Usage-based Token 计数改进：
  - `estimate_current_tokens()` 优先使用 API response 的 usage 数据，仅对新消息使用 chars/4 兜底
  - `usage_exceeds_200k_tokens()` 用于动态模型切换判定
- TUI 中 Thinking 块展示增强（折叠/展开、流式 ThinkingDelta 显示）
- TUI UsageUpdate 事件已包含 `cache_read_input_tokens` / `cache_creation_input_tokens`
- 验证结果：`cargo test --workspace` 通过

**目标**: 补齐影响每一次 API 调用的核心能力 — prompt caching 大幅降低成本和延迟，extended thinking 解锁模型深度推理。这两项是 TypeScript 版本的基础设施级功能。

**背景分析**（来自 TS 源码 `claude.ts`、`tokenEstimation.ts`、`thinking.ts`）:

- TS 版本在 system prompt 块和消息上设置 `cache_control: { type: "ephemeral" }`，实现前缀缓存复用
- Extended thinking 默认开启，支持 adaptive（Opus/Sonnet 4.6）和 budget-based 两种模式
- Thinking 块需要 `signature` 字段才能在后续 turn 中作为 assistant 消息重新发送
- Token 计数使用 API response 中的 usage 数据作为主要来源，chars/4 仅作最后兜底

**产出**:

- `api` crate:
  - `CreateMessageRequest` 新增 `thinking` 字段（`ThinkingConfig`: disabled / enabled with budget / adaptive）
  - `SystemPrompt` 支持带 `cache_control` 的结构化块（`SystemBlock { type, text, cache_control }`）
  - 消息级 `cache_control` 标记 — 在最后一条消息的最后一个内容块上设置
  - `ContentBlock::Thinking` 增加 `signature: Option<String>` 字段
  - 响应解析支持 `server_tool_use` 等新块类型（forward-compatible `#[serde(other)]`）
- `core` crate:
  - `SessionSettings` 新增 `thinking_enabled: bool`（默认 true）
  - Token 计数改进：优先使用 API response 的 usage 数据，chars/4 仅作兜底
  - `CompactionConfig` 使用 usage-based token 计数提升阈值判定精度
- `cli` crate:
  - `QueryLoop` 构建请求时注入 thinking 配置（根据模型自动选择 adaptive 或 budget）
  - `QueryLoop` 构建请求时注入 cache_control 标记
  - Max tokens recovery：当 stop_reason == MaxTokens 时注入恢复消息继续生成（最多 3 次）
  - `--thinking` / `--no-thinking` CLI 参数
- `tui` crate:
  - Thinking 块展示增强（折叠显示 "Thinking..."，可选展开查看思考内容）
  - 状态栏显示 cache hit 信息（`cache_read_input_tokens` / 总 input tokens）

**验收标准**:

- 开启 prompt caching 后，连续对话的 `cache_read_input_tokens` 稳定大于 0
- Extended thinking 在 Opus/Sonnet 4.6 模型上默认开启且正常工作
- Thinking 块的 signature 在多轮对话中正确保留和回传
- Max tokens recovery 能在长输出被截断时自动继续
- `cargo test --workspace` 通过

**依赖**: 迭代 14

---

### 迭代 16：TUI 核心交互增强

**状态**: 已完成

**完成记录**:

- 已实现输入系统重构：
  - 多行 `InputBuffer`：Shift+Enter 换行、Enter 提交、粘贴自动检测多行（Bracketed Paste 支持）
  - 完整光标导航：Home/End、Ctrl+A/Ctrl+E（行首/行尾）、Ctrl+Left/Ctrl+Right（按词移动）
  - 输入历史浏览：Up/Down 导航，500 条上限，持久化到 `~/.config/rust-claude-code/history`
  - 历史浏览时暂存当前输入草稿，返回时恢复
- 已实现 Markdown 基础渲染（`ui.rs`）：
  - 标题（H1-H3）带颜色区分和 BOLD 修饰
  - 有序/无序列表带缩进和标记
  - 代码块（` ``` `）带语言标签和边框渲染（┌─/│/└─ 样式）
  - 段落渲染支持 inline spans（行内代码、粗体、斜体解析）
  - 注：未集成 `syntect` / `tree-sitter` 的语法高亮，代码块内容使用统一颜色
- 已实现键盘快捷键：
  - Escape 取消当前流式输出
  - Ctrl+C 取消流式 / 空输入时退出
  - Ctrl+L 清屏重绘
  - PageUp/PageDown 滚动聊天历史
  - Ctrl+Home/Ctrl+End 跳转到最旧/最新聊天内容
- 已实现 Thinking 块 UI：
  - 流式 ThinkingDelta 显示（实时更新思考文本）
  - 完成后折叠为摘要（"Thought for ~N chars"）
  - Tab 键切换展开/折叠最近的 thinking block
  - `selected_thinking` 状态跟踪
- 验证结果：`cargo test --workspace` 通过

**目标**: 解决日常使用中最痛的交互问题 — 单行输入无法粘贴代码、无法浏览输入历史、Markdown 输出不可读。

**产出**:

- 输入系统重构
  - 多行输入支持（Shift+Enter 换行，Enter 提交；粘贴自动检测多行）
  - 输入历史浏览（Up/Down 导航历史记录，持久化到 `~/.config/rust-claude-code/history`）
  - 光标移动（Home/End/Ctrl+A/Ctrl+E 行内，Ctrl+左右 按词移动）
- Markdown 基础渲染
  - 标题（`#` ~ `###`）加粗或颜色区分
  - 有序/无序列表缩进
  - 代码块（` ``` `）带边框渲染，基础语法高亮（基于 `syntect` 或 `tree-sitter-highlight`）
  - 行内代码（`` ` ``）反色或背景色
  - **粗体** / *斜体* 基础支持
- 键盘快捷键
  - `Escape` 取消当前流式输出
  - `Ctrl+C` 取消流式 / 空输入时退出
  - `Ctrl+L` 清屏
- Thinking 块 UI
  - 流式显示 "Thinking..." spinner
  - 完成后折叠为一行摘要（"Thought for Xs"），点击/快捷键可展开

**验收标准**:

- 多行代码粘贴后能正确提交
- Up/Down 可浏览历史输入
- 代码块有基础语法高亮且不破坏终端
- Escape 可中断流式输出
- 不引入终端恢复问题或明显性能回退

**依赖**: 迭代 10

---

### 迭代 17：配置收敛 + Git 感知 + Slash Commands

**状态**: 已完成

**完成记录**:

- 已实现配置系统收敛：
  - 支持项目级 `.claude/settings.json` 自动发现（`ClaudeSettings::discover_project_settings()`、`load_project()`）
  - Settings 合并优先级：CLI args > `RUST_CLAUDE_*` env > `ANTHROPIC_*` env > project settings > user settings > config defaults
  - `permissions` 字段在 settings 中声明 allow/deny 规则，user + project 层独立合并
  - `ConfigProvenance` 追踪每个配置项的来源（Env / Cli / UserConfig / ProjectSettings / Default）
  - `--settings` 参数支持指定自定义 settings.json 路径
  - 注：未实现 JSON Schema 校验
- 已实现 Git 感知：
  - `GitContextSnapshot`：repo_root、branch、is_clean、recent_commits（`core/src/git.rs`）
  - `collect_git_context()` 收集当前 git 仓库上下文
  - Branch 信息展示在 TUI 状态栏（`StatusUpdate` 事件含 `git_branch`）
  - `gitStatus` 快照注入 system prompt（branch、clean 状态、最近 5 条 commits）
  - 每次查询结束后自动刷新 git context
- 已实现 Slash Commands 增强：
  - `/diff` — 显示当前 git working tree diff
  - `/cost` — 显示会话累计 token 用量与 USD 估算成本
  - `/config` — 显示当前生效的 model/permission/base_url 来源
  - `/clear` 支持可选参数 `keep-context`
  - 注：命令注册仍为静态数组 `SLASH_COMMANDS`，未重构为动态注册框架
- 验证结果：`cargo test --workspace` 通过

**目标**: 统一多来源配置系统，补齐 Git 项目上下文，增加常用 Slash Commands。

**产出**:

- 配置系统收敛
  - 支持项目级 `.claude/settings.json`（与 TS 版本兼容）
  - Settings 合并优先级：CLI args > env vars > project settings > user settings > global defaults
  - 支持 `permissions` 字段在 settings 中声明（allow/deny rules）
  - 配置文件 JSON Schema 校验
- Git 感知
  - Git root 检测（用于 CLAUDE.md 发现、权限路径归一化）
  - Branch 信息读取并展示在 TUI 状态栏
  - `gitStatus` 快照注入 system prompt（当前分支、是否 clean、最近 commits）
- Slash Commands 增强
  - `/diff` — 显示当前 git diff（可选 staged/unstaged）
  - `/cost` — 显示当前会话累计 token 用量与估算成本
  - `/config` — 显示当前生效的配置及其来源
  - `/clear` 增强 — 可选清除消息但保留上下文
  - 命令注册框架重构 — 为后续动态命令（Skills、MCP）预留扩展点

**验收标准**:

- 在有 `.claude/settings.json` 的项目中，配置合并结果正确
- TUI 状态栏显示 git branch 信息
- System prompt 包含 git status 上下文
- 新增 slash commands 在 CLI/TUI 模式下均可用
- 相关测试通过

**依赖**: 迭代 13, 迭代 14

---

### 迭代 18：Hooks 系统

**状态**: 已完成

**完成记录**:

- 已实现 `core` crate 的 `hooks` 模块（`hooks.rs`）：
  - `HookEvent` 枚举：`PreToolUse`、`PostToolUse`、`UserPromptSubmit`、`Stop`、`Notification`
  - `HookConfig` 类型：`type_`（仅支持 `"command"`）、`command`、`timeout`（默认 10s）
  - `HookEventGroup`：`matcher`（工具名匹配）+ hooks 列表
  - `HooksConfig`：`HashMap<String, Vec<HookEventGroup>>`（与 TS settings.json 格式兼容）
  - `HookResult`：`Continue` / `Block { reason }`
  - Hook 输入结构：`PreToolUseInput`、`PostToolUseInput`、`UserPromptSubmitInput`、`StopInput`、`NotificationInput`（JSON 序列化通过 stdin 传递）
- 已实现 Settings 集成：
  - `ClaudeSettings` 新增 `hooks` 字段（`#[serde(default)]`）
  - `ClaudeSettings::merge()` 按事件合并 hook 列表（user hooks 在前，project hooks 在后）
- 已实现 `cli` crate 的 `HookRunner`（`hooks.rs`）：
  - `get_matching_hooks()` — 按事件和 matcher 过滤 hook 配置
  - `execute_command_hook()` — 通过 `tokio::process::Command` spawn shell、stdin JSON 输入、stdout 捕获、超时保护
  - `parse_pre_tool_use_result()` — 解析 JSON 决策或 exit code 语义（0=approve, 1=warn, 2=block）
  - `run_pre_tool_use()` — 编排匹配、执行、结果聚合；首个 block 短路
  - `run_post_tool_use()` / `run_user_prompt_submit()` / `run_stop()` — 信息通知型事件
  - 环境变量注入：`CLAUDE_PROJECT_DIR`、`HOOK_EVENT`
- 已实现 QueryLoop 集成：
  - `with_hook_runner(Arc<HookRunner>)` builder 方法
  - PreToolUse hooks：权限检查通过后、工具执行前触发；block 时跳过执行并返回错误
  - PostToolUse hooks：工具执行后触发（并发和串行工具均支持）
  - UserPromptSubmit hooks：用户提交输入时触发（API 调用前）
  - Stop hooks：QueryLoop 正常结束或达到最大轮次时触发
- 已实现 TUI 集成：
  - `AppEvent::HookBlocked` 事件变体
  - `TuiBridge::send_hook_blocked()` 方法
  - TUI 中显示 hook 阻止信息为系统消息
- 已实现 `/hooks` 斜杠命令：显示已配置的 hooks（按事件分组，显示 matcher 和 command）
- 已在 `main.rs` 中完整接入：从合并后的 settings 构建 `HookRunner`，传递到 print 模式和 TUI 模式
- 注：仅支持 `type: "command"` hook，不支持 prompt/agent/http 类型（加载时 warn 并跳过）
- 注：不支持 `updatedInput`（工具输入修改）、`once`、`async`/`asyncRewake` 等高级特性
- 验证结果：`cargo test --workspace` 通过，353 个测试全部通过

**目标**: 建立自动化扩展点系统，允许用户在关键生命周期节点执行自定义脚本。Hooks 是 MCP、permission 增强等后续功能的基础设施。

**背景分析**（来自 TS 源码 `hooks/`、`settings.json` 配置）:

- TS 版本支持 `PreToolUse`、`PostToolUse`、`Notification`、`UserPromptSubmit`、`Stop` 等 hook 事件
- Hook 通过 `settings.json` 的 `hooks` 字段配置，支持 shell command 和 matcher 条件
- Hook 可返回 `approve`/`deny`/`modify` 决策影响工具执行
- Hook 执行环境包含 tool name、input、output 等上下文变量

**产出**:

- `core` crate:
  - `HookEvent` 枚举：`PreToolUse`、`PostToolUse`、`UserPromptSubmit`、`Notification`、`Stop`
  - `HookConfig` 类型：`event`、`command`、`matcher` (tool name / pattern)、`timeout_ms`
  - `HookResult`：`Continue`、`Block { reason }`、`Modify { ... }`
- `cli` crate:
  - `HookRunner`：根据事件和 matcher 过滤并执行匹配的 hooks
  - Shell command 执行（通过 `tokio::process::Command`），环境变量注入上下文
  - Hook 结果处理（block → 阻止工具执行，modify → 修改工具输入）
  - 超时处理（默认 10s，可配置）
- `QueryLoop` 集成:
  - 工具执行前触发 `PreToolUse` hooks
  - 工具执行后触发 `PostToolUse` hooks
  - 用户提交输入时触发 `UserPromptSubmit` hooks
- Settings 集成:
  - 从 `settings.json` 的 `hooks` 字段加载 hook 配置
  - 项目级和用户级 hooks 合并

**验收标准**:

- 配置 PreToolUse hook 后能在工具执行前稳定触发
- Hook 返回 `block` 时工具执行被阻止
- Hook 超时不会阻塞主流程
- hooks 配置格式与 TS 版本兼容
- 相关测试通过

**依赖**: 迭代 17（依赖配置系统收敛）

---

### 迭代 19：MCP 客户端

**状态**: 已完成

**目标**: 实现 Model Context Protocol 客户端，支持通过 stdio 接入外部工具服务器，大幅扩展可用工具集。

**背景分析**（来自 TS 源码 `services/mcp/`）:

- TS 版本支持 stdio、SSE、StreamableHTTP、WebSocket 四种传输
- MCP 服务器在 `settings.json` 的 `mcpServers` 字段配置
- MCP 工具注册到 ToolRegistry，参与 system prompt 和权限检查
- 支持 OAuth 认证流程（远程服务器）

**产出**:

- `core` crate:
  - `McpServerConfig` 类型：`command`、`args`、`env`、`cwd`
  - `McpTool` 类型：`server_name`、`tool_name`、`description`、`input_schema`
- 新增 `mcp` crate（或在 `tools` 中新增 `mcp` 模块）:
  - JSON-RPC 2.0 over stdio 实现（spawn subprocess, read/write stdin/stdout）
  - `initialize` 握手
  - `tools/list` 获取工具列表
  - `tools/call` 调用工具
  - `McpClient` 管理服务器生命周期（启动、重连、关闭）
  - 多服务器并行管理
- `tools` crate:
  - `McpProxyTool` — 将 MCP 工具包装为本地 `Tool` trait 实现
  - 自动注册到 `ToolRegistry`
- `cli` crate:
  - 从 `settings.json` `mcpServers` 加载 MCP 服务器配置
  - 启动时初始化 MCP 连接，获取工具列表
  - System prompt 中注入 MCP 工具描述
- TUI:
  - `/mcp` slash command — 显示已连接的 MCP 服务器和工具列表
  - 状态栏显示 MCP 服务器连接状态

**验收标准**:

- 可接入至少一个 stdio MCP server（如 `@anthropic-ai/mcp-server-filesystem`）并完成工具调用
- MCP 工具参与正常的权限检查流程
- MCP 服务器崩溃后能优雅处理（日志提示，不影响主流程）
- 服务器重启后支持重连
- 相关测试通过

**依赖**: 迭代 18（Hooks 用于 MCP 工具权限集成）

---

### 迭代 20：Agent Tool + Task System

**状态**: 已完成

**目标**: 实现子代理能力，支持将复杂任务分解为子任务并行处理。Agent 是 Claude Code 处理复杂工程任务的核心能力。

**背景分析**（来自 TS 源码 `tools/AgentTool/`、`tasks/`）:

- TS 版本的 AgentTool 会 fork 一个独立的 QueryLoop 作为子代理
- 子代理有独立的消息历史和工具上下文
- Task 系统提供 create/list/update/get 操作
- Task 状态：pending → in_progress → completed
- 子代理可以被分配 task 并汇报进度

**产出**:

- `tools` crate:
  - `AgentTool` — spawn 独立 QueryLoop 作为子代理
    - Input: `prompt`、`description`、`allowed_tools`（可选）
    - 子代理共享 `AppState` 但有独立消息历史
    - 子代理执行完毕后返回最终文本作为工具结果
  - `TaskCreateTool` — 创建任务（subject、description、status）
  - `TaskListTool` — 列出所有任务及状态
  - `TaskUpdateTool` — 更新任务状态、添加注释
  - `TaskGetTool` — 获取任务详情
- `core` crate:
  - `Task` 类型：`id`、`subject`、`description`、`status`、`owner`、`blocked_by`
  - `TaskStore` — 内存中的任务存储，线程安全（`Arc<Mutex<>>`）
- `tui` crate:
  - Todo panel 改为 Task panel（复用现有 todo 面板，扩展为 task 视图）
  - 子代理执行时在 UI 显示进度

**验收标准**:

- AgentTool 能成功 spawn 子代理执行独立任务
- 子代理可使用文件读写、Bash 等工具
- Task 工具能正确创建、列出、更新任务
- 子代理执行不阻塞主对话（异步）
- 相关测试通过

**依赖**: 迭代 19（Agent 可能使用 MCP 工具）

---

### 迭代 21：LSP Tool + WebFetch/WebSearch

**状态**: 已完成

**目标**: 补齐代码导航和 Web 信息获取能力，扩展工具集的广度。

**产出**:

- `tools` crate — `LspTool`:
  - 支持操作：`goToDefinition`、`findReferences`、`hover`、`documentSymbol`、`workspaceSymbol`
  - LSP 服务器生命周期管理（自动发现并启动对应语言的 LSP server）
  - 支持常见语言：Rust (rust-analyzer)、TypeScript (typescript-language-server)、Python (pyright/pylsp)
  - LSP 协议通信（JSON-RPC over stdio）
  - 结果格式化为用户可读的文本
- `tools` crate — `WebFetchTool`:
  - Input: `url`、`prompt`（用于内容提取/总结）
  - HTTP GET + HTML to Markdown 转换
  - 内容截断（大页面只取前 N 字符）
  - 基础缓存（15 分钟 TTL）
- `tools` crate — `WebSearchTool`:
  - Input: `query`、`allowed_domains`、`blocked_domains`
  - 接入搜索 API（Brave Search / SearXNG / 可配置）
  - 返回格式化的搜索结果列表

**验收标准**:

- LSP goToDefinition 在 Rust 项目中能正确跳转到定义
- WebFetch 能获取并提取网页内容
- WebSearch 能返回相关搜索结果
- 所有新工具与现有权限系统和 QueryLoop 正常协作
- 相关测试通过

**依赖**: 迭代 12

---

### 迭代 22：Memory 系统 + 生态完善

**状态**: 规划中

**目标**: 实现跨会话记忆系统，补齐生态能力的最后一块。Memory 允许 Claude 在多次对话间保持上下文。

**背景分析**（来自 TS 源码 `services/memory/`、system prompt 中的 memory 指令）:

- TS 版本使用 `~/.claude/projects/<project>/memory/` 目录存储记忆文件
- `MEMORY.md` 作为索引文件，每个记忆是独立的 Markdown 文件（带 frontmatter）
- 记忆类型：user、feedback、project、reference
- System prompt 中包含 memory 指令，指导模型何时读写记忆
- 记忆文件路径注入 system prompt

**产出**:

- `core` crate:
  - `MemoryConfig`：memory 目录路径、MEMORY.md 路径
  - `MemoryEntry`：name、description、type、content（frontmatter 解析）
  - `MemoryIndex`：MEMORY.md 解析与更新
- `cli` / `tools`:
  - Memory 目录自动发现（`~/.claude/projects/<project-hash>/memory/`）
  - MEMORY.md 内容注入 system prompt
  - 工具执行中支持读写 memory 文件（通过 FileWrite/FileRead，无需专用工具）
- System prompt 增强:
  - 注入 memory 管理指令（何时保存、何时读取、何时更新）
  - 注入 MEMORY.md 索引内容
- TUI:
  - `/memory` slash command — 显示当前记忆文件列表
- 生态完善:
  - `NotebookEditTool` — 基础 Jupyter notebook 单元格编辑
  - Image 内容块支持（`ContentBlock::Image` — base64 或 URL）
  - `--resume` / `-r` 恢复指定会话（不仅是最新会话）

**验收标准**:

- Memory 文件能正确保存和跨会话读取
- MEMORY.md 索引内容出现在 system prompt 中
- Model 在适当时机自动保存记忆
- 相关测试通过

**依赖**: 迭代 17（依赖配置系统和 system prompt 增强）

---

### 10.6 第二期依赖关系（重新设计）

```text
迭代 14 (Compaction) ──────────────── 迭代 15 (Prompt Caching + Thinking)
                                          │
迭代 10 (TUI 权限) ──────────────── 迭代 16 (TUI 交互增强)
                                          │
迭代 13 (CLAUDE.md) + 迭代 14 ──── 迭代 17 (配置 + Git + Commands)
                                          │
                                     迭代 18 (Hooks)
                                          │
                                     迭代 19 (MCP)
                                          │
                                     迭代 20 (Agent + Task)
                                          │
迭代 12 (Glob + Grep) ──────────── 迭代 21 (LSP + WebFetch + WebSearch)
                                          │
迭代 17 (配置 + Git) ──────────── 迭代 22 (Memory + 生态完善)
```

可并行推进的组合：

- 迭代 15 + 迭代 16（API 增强与 TUI 增强互不阻塞）
- 迭代 21 可在 迭代 18 之后独立推进（LSP/Web 工具不依赖 MCP/Agent）
- 迭代 22 可在 迭代 20 之后独立推进

### 10.7 第二期完成判定（重新设计）

当满足以下条件时，可认为第二期达到阶段性目标：

**基础体验层（迭代 15-17）**:
- Prompt caching 正常工作，连续对话 cache hit 率 > 80%
- Extended thinking 默认开启，thinking 块在 TUI 中正确展示
- 多行输入、输入历史、Markdown 渲染使日常使用体验达到可用水平
- 项目级配置和 Git 感知正常工作

**扩展能力层（迭代 18-20）**:
- Hooks 系统能在工具执行前后触发自定义脚本
- 至少一个 MCP server 可正常接入并完成工具调用
- Agent 子代理能独立执行任务并返回结果

**生态完善层（迭代 21-22）**:
- LSP、WebFetch、WebSearch 工具可用
- Memory 系统支持跨会话记忆保持
- Rust 版本在日常开发中可替代 TS 版本的核心功能
