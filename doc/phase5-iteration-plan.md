# 第五期迭代计划 — 兼容性补齐与上下文安全交付

> 生成时间：2026-05-18
>
> 基于 `doc/feature-gap-analysis.md`、`doc/phase4-iteration-plan.md` 与当前源码状态制定。
>
> 第五期目标：在不让单次 AI 会话上下文膨胀的前提下，逐步补齐与原版 Claude Code 的关键兼容能力，优先覆盖工具协议、任务/团队基础、Skills、MCP 资源与鉴权、worktree 和 headless 输出协议。

---

## 0. 范围边界

### 0.1 本期目标

第五期不追求一次性复刻原版所有生态，而是围绕“可切片交付”的兼容性能力推进：

1. **协议兼容**：文件工具字段兼容、`stream-json` 输出、独立任务工具 schema。
2. **工程编排**：Task 工具族、SendMessage、Team 基础、本地工作流闭环。
3. **扩展入口**：Skills 最小可用、SkillTool、插件与 skill 的边界清晰。
4. **MCP 兼容**：resources、McpAuth、elicitation 分层实现。
5. **隔离开发**：EnterWorktree / ExitWorktree。
6. **安全与体验补齐**：Bash/File 工具细节增强、resume selector、权限 UI 增强。

### 0.2 明确不做

以下仍不纳入本期交付：

| 能力 | 处理 |
|---|---|
| Anthropic 账号 OAuth 登录/登出 | 不做；保留 API key / Bearer / apiKeyHelper 路径 |
| GitHub App / Slack App 等第三方授权安装 | 不做 |
| remote managed settings / enterprise policy sync | 不做 |
| Desktop/Web/Mobile 账号联动 | 不做 |
| 完整 marketplace 远程生态 | 本期只做本地基础能力，不做远程市场 |
| Voice/STT | 不做 |
| IDE 插件本体 | 不做；只保留未来接口空间 |

注意：**MCP 协议层面的 `McpAuth` / MCP OAuth/Auth 是本期范围内，需要实现**。

---

## 1. 上下文安全交付原则

为了避免单次 AI 上下文爆炸，本期所有需求必须按下面规则拆分。

### 1.1 单次实现上下文预算

每个小迭代必须能在一个独立 AI 会话中完成，默认预算如下：

| 项目 | 上限 |
|---|---:|
| 初始必读 Rust 文件 | 8 个以内 |
| 初始必读 restored-src 文件 | 4 个以内 |
| 初始必读文档 | 3 个以内 |
| 单次计划修改文件 | 8 个以内 |
| 单次主要改动 crate | 3 个以内 |
| 单次新增公开 API | 1 组以内 |
| 单次新增工具 | 1 个工具族以内 |
| 单次测试命令 | 3 条以内 |

如果实现过程中发现需要超过这些上限，必须拆成新的子迭代，不允许继续扩大当前迭代。

### 1.2 每个迭代必须包含 Context Pack

每个迭代都记录：

- **Rust 必读**：当前仓库中实现该需求需要先读的文件。
- **原版参考**：`restored-src` 中需要对照的文件。
- **禁止扩散**：本迭代明确不碰的相关能力。
- **验收命令**：本迭代完成后优先运行的最小测试集合。

实现时只读取 Context Pack 中列出的文件；如果必须读额外文件，应优先用精准 grep 定位，避免无边界探索。

### 1.3 切分规则

1. **协议先行，UI 后置**：先让工具/API schema 可用，再做 TUI 展示。
2. **存储先行，工具后置**：任务、团队、消息类能力先做数据模型和本地存储，再接工具。
3. **只做薄桥接**：跨 crate 改动只保留必要接口，不做大规模重构。
4. **每个工具族独立验收**：不要把 Task、Team、Skill、MCP 混在同一轮实现。
5. **不混合“兼容”和“体验”**：例如先做 `stream-json` 协议，再另起迭代做更好看的 TUI。
6. **原版只做行为参考，不做结构照搬**：Rust 实现保持现有 crate 边界。

### 1.4 停止线

遇到以下情况时，应停止当前迭代并拆分：

- 需要同时修改 `api`、`sdk`、`cli`、`tui`、`tools`、`core` 超过 3 个 crate。
- 需要引入新的长期存储格式，但还没有迁移/兼容设计。
- 需要同时改工具 schema 和 TUI 交互。
- 需要一次性读原版超过 4 个目录。
- 单个文件改动超过 500 行且不是纯测试或纯数据结构。

---

## 2. 第五期总体阶段

```
阶段 A（迭代 44-46）: 协议兼容基础
阶段 B（迭代 47-49）: 任务与本地团队基础
阶段 C（迭代 50-52）: Skills 与 Worktree
阶段 D（迭代 53-55）: MCP 资源、鉴权与 elicitation
阶段 E（迭代 56-58）: Headless 输出、安全细节与收口
```

---

## 3. 阶段 A：协议兼容基础

### 迭代 44：文件工具 schema 兼容层

**状态**：规划中

**目标**：让 Rust 文件工具同时接受 `path` 与原版 `file_path`，消除模型/工具提示与原版 schema 的兼容风险。

**范围**：

- `FileRead`、`FileEdit`、`FileWrite` 输入兼容。
- 工具内部继续统一使用 `PathBuf`。
- TUI 摘要和 diff 提取同时识别 `path` / `file_path`。
- 不改变当前工具名。

**不做**：

- 不改动所有工具命名。
- 不做 FileRead 图片/PDF 支持。
- 不做权限规则路径通配增强。

**Context Pack**：

| 类型 | 文件 |
|---|---|
| Rust 必读 | `crates/tools/src/file_read.rs` |
| Rust 必读 | `crates/tools/src/file_edit.rs` |
| Rust 必读 | `crates/tools/src/file_write.rs` |
| Rust 必读 | `crates/tui/src/app.rs` |
| Rust 必读 | `crates/tools/src/tool.rs` |
| 原版参考 | `restored-src/src/tools/FileReadTool/FileReadTool.ts` |
| 原版参考 | `restored-src/src/tools/FileEditTool/types.ts` |
| 原版参考 | `restored-src/src/tools/FileWriteTool/FileWriteTool.ts` |

**验收标准**：

- `FileRead` 接受 `{ "path": "..." }`。
- `FileRead` 接受 `{ "file_path": "..." }`。
- `FileEdit` / `FileWrite` 同时兼容两种字段。
- TUI 工具摘要对两种字段都能显示路径。
- 旧测试继续通过。

**验收命令**：

```bash
cargo test -p rust-claude-tools file_read
cargo test -p rust-claude-tools file_edit
cargo test -p rust-claude-tui summarize_tool_input
```

---

### 迭代 45：工具 schema 契约测试

**状态**：规划中

**目标**：建立内置工具 schema 的快照/契约测试，防止后续改动意外破坏工具输入协议。

**范围**：

- 给核心工具生成稳定 schema 摘要。
- 对 `FileRead` / `FileEdit` / `FileWrite` 的兼容字段做显式测试。
- 对 deferred tool 的 schema 暴露行为做测试。

**不做**：

- 不引入外部 schema 校验服务。
- 不要求与原版 JSON schema 逐字节相等。
- 不改工具实现逻辑。

**Context Pack**：

| 类型 | 文件 |
|---|---|
| Rust 必读 | `crates/tools/src/registry.rs` |
| Rust 必读 | `crates/tools/src/lib.rs` |
| Rust 必读 | `crates/tools/src/tool_search.rs` |
| Rust 必读 | `crates/tools/src/file_read.rs` |
| Rust 必读 | `crates/tools/src/file_edit.rs` |
| Rust 必读 | `crates/tools/src/file_write.rs` |

**验收标准**：

- 核心工具 schema 变更会导致测试失败。
- 文件工具兼容字段有明确测试。
- deferred tool 搜索返回的 schema 可被测试覆盖。

**验收命令**：

```bash
cargo test -p rust-claude-tools schema
cargo test -p rust-claude-tools registry
```

---

### 迭代 46：`stream-json` 输出协议第一版

**状态**：规划中

**目标**：为 headless/SDK 场景增加 `--output-format stream-json`，输出 NDJSON 事件流，先覆盖文本、thinking、tool_use、tool_result、usage、done、error。

**范围**：

- CLI 参数允许 `stream-json`。
- 新增 stream-json output sink。
- 每行输出一个 JSON 对象。
- 事件字段稳定、可测试。

**不做**：

- 不做完整原版 SDK 协议 1:1。
- 不做 remote transport。
- 不做二进制附件/图片事件。

**Context Pack**：

| 类型 | 文件 |
|---|---|
| Rust 必读 | `crates/cli/src/main.rs` |
| Rust 必读 | `crates/sdk/src/output.rs` |
| Rust 必读 | `crates/sdk/src/agent_loop.rs` |
| Rust 必读 | `crates/sdk/src/session.rs` |
| Rust 必读 | `crates/api/src/streaming.rs` |
| 原版参考 | `restored-src/src/cli/print.ts` |
| 原版参考 | `restored-src/src/cli/structuredIO.ts` |
| 原版参考 | `restored-src/src/cli/ndjsonSafeStringify.ts` |

**事件草案**：

```json
{"type":"message_start","session_id":"..."}
{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"..."}}
{"type":"thinking_delta","text":"..."}
{"type":"tool_use","id":"...","name":"FileRead","input":{}}
{"type":"tool_result","tool_use_id":"...","is_error":false,"content":"..."}
{"type":"usage","input_tokens":1,"output_tokens":2}
{"type":"done"}
{"type":"error","message":"..."}
```

**验收标准**：

- `--output-format stream-json` 输出合法 NDJSON。
- 每行可被 `serde_json` 单独解析。
- 工具调用和工具结果事件顺序稳定。
- 普通 `text/json` 输出不受影响。

**验收命令**：

```bash
cargo test -p rust-claude-sdk output
cargo test -p rust-claude-cli stream_json
cargo check --workspace
```

---

## 4. 阶段 B：任务与本地团队基础

### 迭代 47：Task 数据模型与本地存储

**状态**：规划中

**目标**：建立独立于 `AppState.tasks` 的任务列表模型，为原版 Task 工具族、Team 和 Agent 协作打基础。

**范围**：

- 新增 task list 数据结构：subject、description、status、owner、blockedBy、blocks、metadata。
- 支持按 team/session 维度持久化。
- 保留现有 `TaskTool`，但不再作为长期唯一入口。

**不做**：

- 不实现所有 Task 工具。
- 不实现 TeamCreate。
- 不做 TUI Task 面板重构。

**Context Pack**：

| 类型 | 文件 |
|---|---|
| Rust 必读 | `crates/core/src/state.rs` |
| Rust 必读 | `crates/tools/src/task_tool.rs` |
| Rust 必读 | `crates/sdk/src/session.rs` |
| Rust 必读 | `crates/cli/src/session.rs` |
| 原版参考 | `restored-src/src/tools/TaskCreateTool/TaskCreateTool.ts` |
| 原版参考 | `restored-src/src/tools/TaskListTool/TaskListTool.ts` |
| 原版参考 | `restored-src/src/tools/TaskUpdateTool/TaskUpdateTool.ts` |

**验收标准**：

- 可创建、读取、更新、列出任务数据结构。
- 任务可序列化/反序列化。
- 存储路径和文件格式明确。
- 不破坏现有 `TaskTool` 测试。

**验收命令**：

```bash
cargo test -p rust-claude-core task
cargo test -p rust-claude-tools task
```

---

### 迭代 48：Task 工具族最小实现

**状态**：规划中

**目标**：实现独立工具：`TaskCreate`、`TaskGet`、`TaskList`、`TaskUpdate`，先不做后台输出和停止。

**范围**：

- 新增四个工具或一个模块内四个工具定义。
- 注册到 CLI 和 SDK 默认工具集。
- 与迭代 47 的 task store 对接。

**不做**：

- 不实现 `TaskOutput`。
- 不实现 `TaskStop`。
- 不实现 Team owner 自动调度。

**Context Pack**：

| 类型 | 文件 |
|---|---|
| Rust 必读 | `crates/tools/src/task_tool.rs` |
| Rust 必读 | `crates/tools/src/lib.rs` |
| Rust 必读 | `crates/tools/src/registry.rs` |
| Rust 必读 | `crates/cli/src/main.rs` |
| Rust 必读 | `crates/sdk/src/session.rs` |
| 原版参考 | `restored-src/src/tools/TaskCreateTool/prompt.ts` |
| 原版参考 | `restored-src/src/tools/TaskGetTool/prompt.ts` |
| 原版参考 | `restored-src/src/tools/TaskListTool/prompt.ts` |
| 原版参考 | `restored-src/src/tools/TaskUpdateTool/prompt.ts` |

**验收标准**：

- 模型可分别调用 `TaskCreate` / `TaskGet` / `TaskList` / `TaskUpdate`。
- `TaskList` 按 ID 稳定排序。
- `blockedBy` 语义可表示。
- 工具 schema 和 prompt 描述清晰。

**验收命令**：

```bash
cargo test -p rust-claude-tools task_create
cargo test -p rust-claude-tools task_list
cargo check --workspace
```

---

### 迭代 49：本地 SendMessage 与 Team 骨架

**状态**：规划中

**目标**：实现最小本地团队编排骨架，让 `SendMessage`、`TeamCreate`、`TeamDelete` 有可运行语义，但不引入 tmux/iTerm/remote backend。

**范围**：

- Team 元数据：team name、members、agent type、task list 绑定。
- `TeamCreate` 创建本地 team 配置目录。
- `TeamDelete` 删除无活跃成员的本地 team 配置。
- `SendMessage` 先支持同进程/本地 mailbox 文件写入。

**不做**：

- 不启动真实多进程 teammate。
- 不做 tmux/iTerm pane backend。
- 不做远程队列。
- 不做完整团队 UI。

**Context Pack**：

| 类型 | 文件 |
|---|---|
| Rust 必读 | `crates/tools/src/agent_tool.rs` |
| Rust 必读 | `crates/tools/src/tool.rs` |
| Rust 必读 | `crates/tools/src/lib.rs` |
| Rust 必读 | `crates/cli/src/main.rs` |
| Rust 必读 | `crates/core/src/state.rs` |
| 原版参考 | `restored-src/src/tools/SendMessageTool/SendMessageTool.ts` |
| 原版参考 | `restored-src/src/tools/TeamCreateTool/TeamCreateTool.ts` |
| 原版参考 | `restored-src/src/tools/TeamDeleteTool/TeamDeleteTool.ts` |
| 原版参考 | `restored-src/src/utils/teamDiscovery.ts` |

**验收标准**：

- `TeamCreate` 能创建本地 team config。
- `SendMessage` 能写入目标 member mailbox。
- `TeamDelete` 能清理空 team。
- 所有行为都限制在本地配置目录。

**验收命令**：

```bash
cargo test -p rust-claude-tools team
cargo test -p rust-claude-tools send_message
cargo check --workspace
```

---

## 5. 阶段 C：Skills 与 Worktree

### 迭代 50：Skills 目录发现与 frontmatter 解析

**状态**：规划中

**目标**：实现最小 skills loader，支持从本地目录发现 Markdown skill，不执行 skill。

**范围**：

- 发现 `~/.claude/skills`、项目 `.claude/skills`。
- 解析 Markdown frontmatter：name、description、allowed-tools、trigger。
- 返回可用于 slash suggestion 和 SkillTool 的定义列表。

**不做**：

- 不实现 `SkillTool`。
- 不安装远程 skill。
- 不实现 conditional activation。
- 不实现 MCP skill builder。

**Context Pack**：

| 类型 | 文件 |
|---|---|
| Rust 必读 | `crates/sdk/src/plugin.rs` |
| Rust 必读 | `crates/core/src/custom_agents.rs` |
| Rust 必读 | `crates/tui/src/slash.rs` |
| Rust 必读 | `crates/cli/src/main.rs` |
| 原版参考 | `restored-src/src/skills/loadSkillsDir.ts` |
| 原版参考 | `restored-src/src/skills/bundledSkills.ts` |

**验收标准**：

- 能从 user/project skills 目录列出 skill。
- frontmatter 解析错误可报告但不中断启动。
- 同名 skill 的覆盖规则明确。

**验收命令**：

```bash
cargo test -p rust-claude-core skill
cargo test -p rust-claude-sdk skill
```

---

### 迭代 51：SkillTool 最小执行

**状态**：规划中

**目标**：实现 `Skill` / `SkillTool`，允许模型按 skill 名称加载并执行本地 skill prompt。

**范围**：

- 新增 `SkillTool`。
- 输入：`skill`、`args`。
- 输出：skill 的 prompt 内容或派发后的 prompt。
- 接入 ToolRegistry。

**不做**：

- 不做远程安装。
- 不做 skill 内嵌 hooks。
- 不做复杂 artifact 生成。

**Context Pack**：

| 类型 | 文件 |
|---|---|
| Rust 必读 | `crates/tools/src/tool.rs` |
| Rust 必读 | `crates/tools/src/lib.rs` |
| Rust 必读 | `crates/tools/src/registry.rs` |
| Rust 必读 | `crates/sdk/src/plugin.rs` |
| Rust 必读 | `crates/cli/src/main.rs` |
| 原版参考 | `restored-src/src/tools/SkillTool/SkillTool.ts` |
| 原版参考 | `restored-src/src/tools/SkillTool/prompt.ts` |

**验收标准**：

- `SkillTool` 可按名称找到本地 skill。
- skill prompt 能包含 args 替换。
- 缺失 skill 返回明确错误。
- slash suggestion 可显示本地 skill 名称。

**验收命令**：

```bash
cargo test -p rust-claude-tools skill
cargo test -p rust-claude-tui slash
```

---

### 迭代 52：EnterWorktree / ExitWorktree

**状态**：规划中

**目标**：实现 git worktree 隔离工具，支持创建、进入、退出、保留或移除当前会话工作树。

**范围**：

- `EnterWorktree`：创建新 worktree 或进入已存在 worktree。
- `ExitWorktree`：返回原目录，支持 keep/remove。
- 维护 session 中的 worktree state。
- 与 cwd、git context、CLAUDE.md 重新加载边界对齐。

**不做**：

- 不做远程 worktree。
- 不做 tmux 自动附加。
- 不做复杂冲突处理 UI。

**Context Pack**：

| 类型 | 文件 |
|---|---|
| Rust 必读 | `crates/core/src/git.rs` |
| Rust 必读 | `crates/core/src/state.rs` |
| Rust 必读 | `crates/tools/src/bash.rs` |
| Rust 必读 | `crates/tools/src/lib.rs` |
| Rust 必读 | `crates/cli/src/main.rs` |
| 原版参考 | `restored-src/src/tools/EnterWorktreeTool/EnterWorktreeTool.ts` |
| 原版参考 | `restored-src/src/tools/ExitWorktreeTool/ExitWorktreeTool.ts` |
| 原版参考 | `restored-src/src/utils/worktree.ts` |

**验收标准**：

- git 仓库中可创建 `.claude/worktrees/<name>` worktree。
- 进入 worktree 后后续工具 cwd 使用 worktree。
- `ExitWorktree keep` 保留目录和分支。
- `ExitWorktree remove` 在安全条件满足时删除。
- 有未提交改动时默认拒绝 remove。

**验收命令**：

```bash
cargo test -p rust-claude-tools worktree
cargo test -p rust-claude-core git
```

---

## 6. 阶段 D：MCP 资源、鉴权与 elicitation

### 迭代 53：MCP resources list/read

**状态**：规划中

**目标**：实现 `ListMcpResources` 与 `ReadMcpResource`，让 Rust 版能浏览和读取 MCP server 暴露的 resources。

**范围**：

- MCP protocol 增加 `resources/list`。
- MCP protocol 增加 `resources/read`。
- 新增两个工具并注册。
- 资源内容转为稳定 ToolResult 文本。

**不做**：

- 不做 MCP auth。
- 不做 elicitation。
- 不做 resource subscription。

**Context Pack**：

| 类型 | 文件 |
|---|---|
| Rust 必读 | `crates/mcp/src/protocol.rs` |
| Rust 必读 | `crates/mcp/src/manager.rs` |
| Rust 必读 | `crates/mcp/src/transport.rs` |
| Rust 必读 | `crates/tools/src/mcp_proxy.rs` |
| Rust 必读 | `crates/tools/src/lib.rs` |
| 原版参考 | `restored-src/src/tools/ListMcpResourcesTool/ListMcpResourcesTool.ts` |
| 原版参考 | `restored-src/src/tools/ReadMcpResourceTool/ReadMcpResourceTool.ts` |
| 原版参考 | `restored-src/src/services/mcp/client.ts` |

**验收标准**：

- 可列出测试 MCP server resources。
- 可读取指定 resource URI。
- 空 resource 列表有明确输出。
- server 不支持 resources 时返回清晰错误。

**验收命令**：

```bash
cargo test -p rust-claude-mcp resources
cargo test -p rust-claude-tools mcp_resource
```

---

### 迭代 54：McpAuth 最小兼容

**状态**：规划中

**目标**：实现 MCP 协议层 auth 工具与 token 存储骨架，支持需要鉴权的 MCP server 进入授权流程。

**范围**：

- 新增 `McpAuth` 工具。
- 支持 server auth metadata 检测。
- 支持打开/返回授权 URL 的 headless 表达。
- 支持 token 存储接口与最小 refresh/revoke 占位。

**不做**：

- 不做 Anthropic 账号登录。
- 不做第三方 App 安装。
- 不做系统浏览器强依赖；TUI/CLI 可先输出 URL。

**Context Pack**：

| 类型 | 文件 |
|---|---|
| Rust 必读 | `crates/mcp/src/protocol.rs` |
| Rust 必读 | `crates/mcp/src/manager.rs` |
| Rust 必读 | `crates/core/src/mcp_config.rs` |
| Rust 必读 | `crates/tools/src/lib.rs` |
| Rust 必读 | `crates/tools/src/tool.rs` |
| 原版参考 | `restored-src/src/tools/McpAuthTool/McpAuthTool.ts` |
| 原版参考 | `restored-src/src/services/mcp/auth.ts` |
| 原版参考 | `restored-src/src/services/mcp/oauthPort.ts` |

**验收标准**：

- `McpAuth` 能识别目标 server。
- 对需要 OAuth 的 server 返回授权指引或 auth URL。
- token 存储路径和格式明确。
- 不影响无需鉴权的 MCP server。

**验收命令**：

```bash
cargo test -p rust-claude-mcp auth
cargo test -p rust-claude-tools mcp_auth
```

---

### 迭代 55：MCP elicitation 最小交互

**状态**：规划中

**目标**：支持 MCP server 在 tool/resource 调用过程中向用户发起 elicitation，并通过 TUI/headless 返回结果。

**范围**：

- MCP client 能识别 elicitation 请求。
- 复用 `AskUserQuestion` 的 UI 回调语义。
- headless 下返回明确不可交互错误或默认策略。

**不做**：

- 不做复杂表单 UI。
- 不做长期事件订阅。
- 不做远程控制。

**Context Pack**：

| 类型 | 文件 |
|---|---|
| Rust 必读 | `crates/mcp/src/protocol.rs` |
| Rust 必读 | `crates/mcp/src/manager.rs` |
| Rust 必读 | `crates/tools/src/ask_user_question.rs` |
| Rust 必读 | `crates/tools/src/tool.rs` |
| Rust 必读 | `crates/sdk/src/agent_loop.rs` |
| 原版参考 | `restored-src/src/services/mcp/elicitationHandler.ts` |
| 原版参考 | `restored-src/src/components/mcp/ElicitationDialog.tsx` |

**验收标准**：

- MCP elicitation 请求可转为用户问题。
- TUI 可返回用户选择。
- headless 行为明确、可测试。

**验收命令**：

```bash
cargo test -p rust-claude-mcp elicitation
cargo test -p rust-claude-tools ask_user_question
```

---

## 7. 阶段 E：Headless 输出、安全细节与收口

### 迭代 56：Bash 安全语义增强第一批

**状态**：规划中

**目标**：补齐最常见的 Bash read-only 识别和危险命令提示，提升权限体验与安全性。

**范围**：

- read-only 命令分类：`git status`、`git diff`、`ls`、`pwd`、`grep`、`find` 等。
- destructive command warning：`rm -rf`、`chmod -R`、`chown -R`、`git reset --hard`、`git clean -fd` 等。
- permission check 中可使用分类结果。

**不做**：

- 不实现完整 bash parser。
- 不做 sed edit parser。
- 不做 PowerShell。

**Context Pack**：

| 类型 | 文件 |
|---|---|
| Rust 必读 | `crates/tools/src/bash.rs` |
| Rust 必读 | `crates/core/src/permission.rs` |
| Rust 必读 | `crates/sdk/src/agent_loop.rs` |
| 原版参考 | `restored-src/src/tools/BashTool/commandSemantics.ts` |
| 原版参考 | `restored-src/src/tools/BashTool/destructiveCommandWarning.ts` |
| 原版参考 | `restored-src/src/tools/BashTool/readOnlyValidation.ts` |

**验收标准**：

- 明确 read-only 命令在默认模式下减少不必要确认。
- 高风险命令会产生明确 warning 或需要确认。
- 分类函数有独立单元测试。

**验收命令**：

```bash
cargo test -p rust-claude-tools bash
cargo test -p rust-claude-core permission
```

---

### 迭代 57：FileRead 安全与格式增强第一批

**状态**：规划中

**目标**：增强 FileRead 的安全边界和大文件体验，先覆盖 device path、binary guard、size limit、相似路径提示。

**范围**：

- 阻止读取常见阻塞设备路径。
- binary 文件提示而非直接读成 UTF-8。
- 文件大小限制和分页提示。
- 文件不存在时给相似路径建议。

**不做**：

- 不做图片读取。
- 不做 PDF 解析。
- 不做 notebook 读取增强。

**Context Pack**：

| 类型 | 文件 |
|---|---|
| Rust 必读 | `crates/tools/src/file_read.rs` |
| Rust 必读 | `crates/core/src/file_state_cache.rs` |
| 原版参考 | `restored-src/src/tools/FileReadTool/FileReadTool.ts` |
| 原版参考 | `restored-src/src/tools/FileReadTool/limits.ts` |
| 原版参考 | `restored-src/src/utils/file.js` |

**验收标准**：

- `/dev/zero` 等路径被拒绝。
- binary 文件返回可理解提示。
- 大文件默认要求 offset/limit。
- 文件不存在时提供相似路径建议。

**验收命令**：

```bash
cargo test -p rust-claude-tools file_read
```

---

### 迭代 58：第五期收口与文档回写

**状态**：规划中

**目标**：核对第五期已完成项，更新差异分析和后续计划，避免文档再次滞后。

**范围**：

- 更新 `doc/feature-gap-analysis.md`。
- 更新本文件各迭代状态。
- 为未完成项重新排期。
- 记录新的“不做/延后”边界。

**不做**：

- 不新增功能。
- 不做大规模重构。

**Context Pack**：

| 类型 | 文件 |
|---|---|
| 文档必读 | `doc/feature-gap-analysis.md` |
| 文档必读 | `doc/phase5-iteration-plan.md` |
| 文档必读 | `doc/phase4-iteration-plan.md` |
| Rust 校验 | `crates/tools/src/lib.rs` |
| Rust 校验 | `crates/cli/src/main.rs` |
| Rust 校验 | `crates/sdk/src/lib.rs` |

**验收标准**：

- 文档状态与代码状态一致。
- 未完成项有下一步归属。
- 删除已失效的旧结论。

**验收命令**：

```bash
git diff -- doc/feature-gap-analysis.md doc/phase5-iteration-plan.md
cargo check --workspace
```

---

## 8. 依赖关系

```text
阶段 A
  44 文件工具 schema 兼容
    └── 45 工具 schema 契约测试
  46 stream-json 输出协议（可并行）

阶段 B
  47 Task 数据模型与存储
    └── 48 Task 工具族最小实现
          └── 49 本地 SendMessage 与 Team 骨架

阶段 C
  50 Skills 发现与解析
    └── 51 SkillTool 最小执行
  52 Worktree 工具（可独立）

阶段 D
  53 MCP resources list/read
  54 McpAuth 最小兼容
    └── 55 MCP elicitation 最小交互

阶段 E
  56 Bash 安全语义增强
  57 FileRead 安全与格式增强
  58 收口与文档回写
```

可并行推进：

- 44 与 46 可并行。
- 47 与 50 可并行。
- 52 可独立推进。
- 53 与 54 可并行设计，但实现时建议分开。
- 56 与 57 可并行。

---

## 9. 阶段完成判定

### 阶段 A 完成标准

- [ ] 文件工具同时兼容 `path` 与 `file_path`。
- [ ] 工具 schema 有契约测试保护。
- [ ] `--output-format stream-json` 可输出合法 NDJSON。

### 阶段 B 完成标准

- [ ] 独立 Task 数据模型和本地存储可用。
- [ ] `TaskCreate/Get/List/Update` 可用。
- [ ] 本地 `SendMessage`、`TeamCreate`、`TeamDelete` 骨架可用。

### 阶段 C 完成标准

- [ ] 本地 skills 可发现和解析。
- [ ] `SkillTool` 可执行本地 skill。
- [ ] `EnterWorktree` / `ExitWorktree` 可用。

### 阶段 D 完成标准

- [ ] MCP resources 可 list/read。
- [ ] `McpAuth` 有最小可用流程。
- [ ] MCP elicitation 可转为用户交互。

### 阶段 E 完成标准

- [ ] Bash 常见安全语义有测试覆盖。
- [ ] FileRead 常见安全边界有测试覆盖。
- [ ] 差异文档与第五期计划状态同步。

---

## 10. 单次 AI 会话执行模板

每次开始实现某个迭代时，建议直接使用下面模板，避免上下文失控：

```text
目标：实现第五期迭代 XX：<名称>。

只读取这些文件：
- <从 Context Pack 复制 Rust 必读>
- <从 Context Pack 复制原版参考>

本轮不做：
- <从禁止扩散复制>

验收命令：
- <从验收命令复制>

要求：
- 如果发现需要新增超过 8 个修改文件，停止并拆分。
- 如果需要读 Context Pack 外超过 3 个文件，先说明原因。
- 不做与本迭代无关的重构。
```

---

## 11. 风险与缓解

| 风险 | 典型表现 | 缓解 |
|---|---|---|
| 上下文膨胀 | 一个迭代同时读工具、TUI、SDK、原版多个目录 | 严格执行 Context Pack 和停止线 |
| 原版结构过大 | 照搬 TS 文件组织导致 Rust crate 边界混乱 | 只对齐行为，不复制结构 |
| 工具 schema 反复变更 | 模型提示、TUI、测试互相打架 | 先做迭代 45 契约测试 |
| Team 需求外溢 | 从本地 mailbox 扩散到 tmux/remote/swarm | 迭代 49 只做本地骨架 |
| MCP Auth 复杂度高 | OAuth flow、token refresh、server metadata 混在一轮 | 迭代 54 只做最小兼容，后续再补完整浏览器回调 |
| Skills 变成插件系统重写 | 一次性做 marketplace、install、hooks | 阶段 C 只做本地 skill 发现和执行 |

---

## 12. 第五期之后的候选方向

第五期完成后，再评估以下方向：

1. `TaskOutput` / `TaskStop` 与后台进程统一。
2. Team 多后端：tmux、iTerm、远程。
3. MCP WebSocket transport 和 subscription。
4. FileRead 图片/PDF/notebook 增强。
5. PowerShell 工具。
6. Permission path-scoped rules 与 shadowed rule detection。
7. Plugin 本地 install/remove 完整化。
8. OS-level sandbox 完整实现。
9. IDE 集成协议层。
10. Remote sessions 本地传输层。
