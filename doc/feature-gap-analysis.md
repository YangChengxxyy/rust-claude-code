# Rust Claude Code vs 原版 Claude Code 功能差异分析

> 更新时间：2026-05-18
>
> 本文基于当前 `rust-claude-code` 仓库与 `/Users/cc/projects/claude-code-sourcemap/restored-src` 源码对照更新。旧版文档中关于 `WebSearch`、`AskUserQuestion`、`Monitor`、plan mode、TUI streaming、MCP HTTP/SSE 等缺失结论已经过期。
>
> 范围说明：账号登录/登出、第三方 App 授权安装、remote managed settings 等产品化云端账号与组织策略能力明确标记为“不做”。MCP 协议层面的 `McpAuth` / MCP OAuth/Auth 属于兼容性范围，需要实现。

---

## 一、当前 Rust 版已基本具备的能力

### 1.1 核心工具与运行时注册

Rust 版当前运行时注册的核心工具集中在 `crates/cli/src/main.rs` 与 `crates/sdk/src/session.rs`，主要包括：

| 工具/能力 | Rust 版状态 | 备注 |
|---|---|---|
| `Bash` | 已实现 | 支持命令执行、超时和基础权限检查 |
| `FileRead` | 已实现 | schema 使用 `path` 字段 |
| `FileEdit` | 已实现 | schema 使用 `path` 字段；含文件状态缓存/stale 检查 |
| `FileWrite` | 已实现 | schema 使用 `path` 字段；含文件状态缓存/stale 检查 |
| `Glob` | 已实现 | 文件模式搜索 |
| `Grep` | 已实现 | 文本搜索 |
| `NotebookEdit` | 已实现 | 基础 notebook cell 编辑 |
| `TodoWrite` | 已实现 | 会话内 todo 更新 |
| `WebFetch` | 已实现 | HTTP fetch 类工具 |
| `WebSearch` | 已实现 | 作为 deferred tool 注册 |
| `AskUserQuestion` | 已实现 | 可接入 TUI 的用户提问回调 |
| `AgentTool` | 已实现 | 支持子代理基础执行 |
| `LspTool` | 已实现 | 支持 LSP go-to-definition、references 等操作 |
| `Monitor` | 已实现 | 后台/监控类工具的基础实现 |
| `EnterPlanMode` / `ExitPlanMode` | 已实现 | 作为工具注册，不再只是 slash mode 切换 |
| `ToolSearch` | 已实现 | 支持 deferred tool 搜索与 `select:` 选择 |
| `AutoMemory` | 已实现 | 与 Rust memory store 集成 |
| `McpProxy` | 已实现 | 将 MCP server tools 暴露为 `mcp__server__tool` |
| `Task` | 部分实现 | 单个合并式工具，支持 create/list/update/get；不等价于原版多工具任务系统 |

### 1.2 Agent loop / Streaming / Compaction

| 功能 | Rust 版状态 | 备注 |
|---|---|---|
| Agentic query loop | 已实现 | 多轮模型调用与工具执行 |
| SSE 流式响应 | 已实现 | API 层和 TUI bridge 都已有流式事件 |
| Streaming tool execution | 已实现 | `StreamingToolExecutor` 支持工具流式触发和中断行为 |
| 工具并发模型 | 已实现 | concurrent-safe 工具可并行，其他串行 |
| Max tokens 自动续接 | 已实现 | 最多自动恢复 3 次 |
| Overload 退避 | 已实现 | 连续 529 overload 有重试状态 |
| Reactive compaction | 已实现 | 支持默认、aggressive、preserve-recent 等策略 |
| Memory 注入 | 已实现 | 会话开始扫描 memory store 并选择相关记忆 |
| Session JSONL 持久化 | 已实现 | 支持 `--continue` / `--resume` 基础恢复 |

### 1.3 CLI 与配置

| 功能 | Rust 版状态 | 备注 |
|---|---|---|
| `--model` / `--mode` / `--max-turns` | 已实现 | `--mode` 额外支持 `auto` |
| `--print` | 已实现 | 非交互模式 |
| `--output-format text/json/stream-json` | 已实现 | `stream-json` 输出 NDJSON 事件流（迭代 46） |
| system prompt 参数 | 已实现 | 支持覆盖与追加、文件输入 |
| allowed/disallowed tools | 已实现 | 工具名精确过滤 |
| `--sandbox` / `--sandbox-no-network` | 部分实现 | 参数与配置层已有，真正 OS-level sandbox 仍不足 |
| `--provider anthropic/bedrock/vertex` | 部分实现 | CLI 参数存在，实际 provider 路由仍需校验和完善 |
| `--thinking` / `--no-thinking` | 已实现 | 扩展 thinking 参数链 |
| `--continue` / `--resume` | 已实现 | 基础会话恢复 |
| settings 合并 | 部分实现 | 支持 user/project/CLI 层；企业 managed/remote settings 仍缺 |

### 1.4 TUI

| 功能 | Rust 版状态 | 备注 |
|---|---|---|
| Ratatui 基础 UI | 已实现 | `app.rs` / `ui.rs` 已形成完整 TUI 主体 |
| Token streaming 渲染 | 已实现 | 旧文档中“仅最终文本”已过期 |
| Thinking streaming | 已实现 | bridge 有 thinking start/delta/complete 事件 |
| Tool use / tool result 展示 | 已实现 | 支持工具输入流、工具结果和错误展示 |
| Diff 渲染 | 已实现 | 有 `tui/src/diff.rs` |
| Syntax highlighting | 已实现 | 基于 syntect 的高亮模块 |
| Dark/Light theme | 已实现 | 支持内置 dark/light |
| 自定义主题加载 | 部分实现 | 有 reload/load-custom 路径，生态不如原版完整 |
| 权限/信任/用户问题对话框 | 部分实现 | 可用但不如原版 React/Ink UI 丰富 |
| Slash suggestions | 已实现 | 支持命令和 skill suggestion 分组展示 |

### 1.5 MCP / Plugin / Custom Agents / Hooks / Memory

| 功能 | Rust 版状态 | 备注 |
|---|---|---|
| MCP stdio transport | 已实现 | 支持进程启动和 JSON-RPC framing |
| MCP HTTP transport | 已实现 | 基础 request/notification |
| MCP SSE transport | 已实现 | 基础 SSE response 解析 |
| MCP tools discovery/call | 已实现 | `tools/list` 与 `tools/call` |
| Plugin manifest 加载 | 部分实现 | 支持本地 `plugin.json` |
| Plugin MCP/custom agents/slash commands | 部分实现 | 可从 manifest 注入 |
| Custom agents `.claude/agents/*.md` | 已实现 | 支持 frontmatter + prompt body |
| Hooks | 已实现 | 支持 PreToolUse、PostToolUse、UserPromptSubmit、Stop、Notification、SessionStart、SessionEnd |
| Memory store | 已实现 | frontmatter、相关性选择、去重和 auto memory 基础能力 |

---

## 二、与原版仍存在的主要缺口

### 2.1 工具生态缺口

原版 `restored-src/src/tools/` 仍有一批 Rust 版未实现或未等价实现的工具：

| 缺失/不等价工具 | 优先级 | 说明 |
|---|---:|---|
| `EnterWorktree` / `ExitWorktree` | 高 | 原版支持显式进入/退出 git worktree 隔离会话 |
| `SkillTool` | 高 | 原版可加载并执行 skills；Rust 目前没有完整 skill 系统 |
| `SendMessage` | 高 | 原版支持 agent/team 间消息传递 |
| `TeamCreate` / `TeamDelete` | 高 | 原版支持多代理团队编排 |
| `TaskCreate` / `TaskGet` / `TaskList` / `TaskUpdate` / `TaskOutput` / `TaskStop` | 高 | Rust 只有合并式 `Task` 工具，缺原版任务列表、任务所有权、依赖和后台任务输出语义 |
| `CronCreate` / `CronDelete` / `CronList` | 中 | 原版支持会话内定时任务 |
| `McpAuth` | 中 | MCP 协议层 OAuth/鉴权流程缺失，需要实现 |
| `ListMcpResources` / `ReadMcpResource` | 中 | MCP resources/prompts 浏览缺失 |
| `PowerShell` | 中 | Windows 原生命令工具缺失 |
| `RemoteTrigger` | 中 | 远端触发/远程会话相关能力缺失 |
| `REPLTool` | 低 | 原版 REPL primitive tools 缺失 |
| `SleepTool` | 低 | 原版调度/等待辅助工具缺失 |
| `SyntheticOutputTool` | 低 | 原版合成输出工具缺失 |
| `BriefTool` | 低 | 原版 brief/附件上传相关工具缺失 |
| `ConfigTool` | 低 | 原版工具化配置管理缺失 |

### 2.2 工具 schema 差异

Rust 文件工具使用 `path` 字段：

- `FileRead`: `path`, `offset`, `limit`
- `FileEdit`: `path`, `old_string`, `new_string`, `replace_all`
- `FileWrite`: `path`, `content`

原版 Claude Code 的文件工具仍以 `file_path` 为主，例如 `FileEditTool` 的 `toAutoClassifierInput` / `getPath` 都读取 `input.file_path`。

影响：

1. Rust TUI 和工具实现内部应统一使用 `path`。
2. 如果目标是更高 Claude Code 兼容性，后续需要决定是否改回 `file_path`，或提供兼容别名。
3. 当前 `crates/tui/src/app.rs` 中把文件工具摘要/diff 提取改为 `path` 是和 Rust 工具 schema 对齐的修正。

### 2.3 Bash / File 工具细节仍比原版简化

原版 Bash/File 工具包含大量安全和体验逻辑，Rust 当前仍有差距：

| 方面 | 原版能力 | Rust 版差异 |
|---|---|---|
| Bash command semantics | 识别 read-only、git、destructive、sed edit 等语义 | Rust 版较基础 |
| Bash safety classifier | dangerous patterns、destructive warning、sandbox decision | Rust 版未完整移植 |
| Bash cwd 行为 | 原版 session 中 shell cwd 更接近持久化体验 | Rust 版仍需确认/完善跨命令 cwd |
| FileRead | 原版支持图片、PDF、notebook、binary/device path guard、token/file size 限制、相似路径提示、skills path 激活 | Rust 版主要是文本/目录读取 |
| FileEdit | 原版支持 LSP diagnostic 清理、IDE 通知、git diff 获取、quote style preserve、team memory secret guard、settings 文件特殊校验 | Rust 版主要是字符串替换和 stale 检查 |
| FileWrite | 原版有更完整权限、diff、文件历史和 IDE 集成 | Rust 版较基础 |

---

## 三、Slash 命令差异

Rust 版当前内置约 32 个 slash 命令，主要包括：

- 会话：`/clear`、`/compact`、`/resume`、`/exit`
- 模式/模型：`/mode`、`/model`、`/plan`
- 信息诊断：`/config`、`/cost`、`/diff`、`/doctor`、`/status`、`/context`、`/keybindings`
- Memory / Hooks / MCP / Permissions：`/memory`、`/hooks`、`/mcp`、`/permissions`
- 项目/会话管理：`/init`、`/rename`、`/rewind`、`/recap`、`/add-dir`、`/branch`
- 审查/Agents/Auth/Preferences/Misc：`/review`、`/agents`、`/login`、`/logout`、`/effort`、`/theme`、`/export`、`/copy`、`/help`、`/todo`

仍缺原版大量命令或完整交互流程：

| 缺失/简化命令 | 优先级 | 说明 |
|---|---:|---|
| `/skills` | 高 | 原版 skills 发现/安装/管理入口 |
| `/tasks` | 高 | 原版任务系统入口，Rust `/todo` 只是提示 |
| `/resume` 交互选择器 | 高 | Rust 目前简化为 `ListSessions`，未实现完整选择 UI |
| `/login` / `/logout` OAuth 流程 | 不做 | 账号 OAuth 登录登出属于产品化云端鉴权范围，明确不做 |
| `/permissions` 完整交互管理 | 中 | Rust 有入口，原版规则解释、shadowed rule、分类器更完整 |
| `/sandbox-toggle` | 中 | Rust 有 sandbox 参数但缺交互 toggle |
| `/output-style` | 中 | 原版支持 output styles，Rust 缺 |
| `/voice` | 中 | 原版语音输入，Rust 缺 |
| `/vim` | 中 | 原版 vim 模式，Rust 缺 |
| `/remote-env` / `/remote-setup` / `/teleport` | 中 | 原版远程会话能力，Rust 缺 |
| `/install-github-app` / `/install-slack-app` | 低 | 第三方集成缺失 |
| `/feedback` / `/share` / `/stickers` / `/mobile` / `/desktop` | 低 | 产品化周边命令缺失 |
| `/stats` / `/extra-usage` / `/usage` | 低 | 使用统计和用量报告不完整 |
| `/thinkback` / `/passes` / `/autofix-pr` / `/bughunter` | 低 | 内部/高级工作流缺失 |

---

## 四、平台、服务与生态集成差异

### 4.1 MCP

Rust 已支持 MCP stdio/http/sse transport、tools discovery 和 tool call，仍缺：

- MCP OAuth / step-up auth / token revoke（需要实现 `McpAuth`）
- MCP WebSocket transport
- MCP resources/prompts 列表和读取
- MCP elicitation 交互
- MCP server approval UI
- MCP official registry / channel allowlist / channel permissions
- MCP skill builders
- MCP server entrypoint 的完整对等能力

### 4.2 Skills 与 Plugin Marketplace

Rust 有本地 plugin manifest 扫描和基础 install/remove，但原版还包含：

- `src/skills/loadSkillsDir.ts` 的 Markdown skill frontmatter 加载
- bundled skills
- conditional skill activation
- `SkillTool`
- skill improvement survey
- plugin marketplace
- plugin autoupdate
- plugin policy / trust / validation
- managed plugins
- builtin marketplace
- remote/official marketplace 拉取

### 4.3 Custom Agents

Rust 已支持 `.claude/agents/*.md` 基础解析和 plugin custom agents，但原版仍更多：

- built-in agents 完整定义与覆盖规则
- agent wizard / editor / color / model selector
- plugin agents 与 MCP requirements 过滤
- agent memory snapshot
- subagent resume/fork 展示细节
- 多 agent UI 和队列/任务集成

### 4.4 Remote / Team / IDE / Voice

这些是当前最大缺口：

| 领域 | 原版能力 | Rust 现状 |
|---|---|---|
| Remote sessions | bridge、direct connect、remote session manager、permission bridge、remote settings | 基本缺失 |
| Team / Swarm | teammate、mailbox、team memory、tmux/iTerm backend、leader permission bridge | 基本缺失 |
| IDE 集成 | VS Code/JetBrains/Cursor、selection、diff in IDE、IDE status、LSP plugin recommendation | 基本缺失 |
| Voice | audio capture、voice hooks、voice UI | 缺失 |
| Desktop/Web/Mobile | desktop handoff、Chrome、mobile、claude.ai/code 相关入口 | 缺失 |

### 4.5 Sandbox 与企业策略

Rust 当前有 sandbox 配置和命令包装基础，但原版还有：

- 更完整的 platform sandbox adapter
- sandbox permission UI
- sandbox violation 展示
- bypass permissions killswitch
- remote managed settings
- enterprise policy / settings sync
- plugin policy 和 managed plugins

### 4.6 明确不做的原版产品化能力

以下原版能力属于 Anthropic 产品账号、组织策略或第三方授权集成，不作为 Rust 重写版目标；MCP 协议层鉴权不在此列，仍需要实现：

| 能力 | 原版示例 | Rust 版处理 |
|---|---|---|
| 账号 OAuth 登录/登出 | `/login`、`/logout`、OAuth refresh | 不做；保留 API key / Bearer / apiKeyHelper 等本地凭据路径 |
| 第三方 App 授权安装 | GitHub App、Slack App 安装命令 | 不做 |
| Remote managed settings | 远端组织策略、settings sync、enterprise policy | 不做 |
| Desktop/Web/Mobile 账号联动 | desktop handoff、mobile、claude.ai/code 账号桥接 | 不做 |

---

## 五、当前存在的实现差异

| 方面 | 原版行为 | Rust 版当前行为 | 影响 |
|---|---|---|---|
| 文件工具字段 | 多数文件工具使用 `file_path` | Rust 使用 `path` | 与原版 schema 不兼容，但 Rust 内部一致 |
| 默认 max rounds | 原版更接近用户可中断的长循环 | Rust `QueryLoop::new` 默认 8 轮 | 复杂任务可能提前停止，除非设置 `--max-turns` |
| `--output-format` | text/json/stream-json | text/json/stream-json | `stream-json` 为第一版 NDJSON 事件流，未做原版 SDK 协议 1:1（迭代 46） |
| Task 工具 | 多个独立工具 + 共享 task list + owner/dependency | 单个 `Task` 工具（内存）+ 独立持久化 task-list 模型（`core/task_list.rs`，迭代 47）与 `TaskCreate/Get/List/Update` 工具族（`tools/task_tools.rs`，迭代 48，按 session scope 落盘）；尚未实现 `TaskOutput`/`TaskStop` 与 Team owner 调度 | 多代理协作不可等价 |
| Plugin 系统 | marketplace、policy、autoupdate、bundled plugins | 本地 manifest 基础加载 | 生态能力差距大 |
| Skills | Markdown skill 系统 + bundled skills + `SkillTool` | 缺失 | slash skill 和工具 skill 不兼容 |
| MCP | transport + auth + resources + elicitation + UI | transport + tools 基础调用 | MCP 复杂服务不可完整使用 |
| Permission | 完整规则引擎、解释、shadowed detection、classifier/yolo、remote/team 场景 | 基础模式、规则和 hooks | 审批体验和安全提示少 |
| Hooks | 事件更多，集成插件、team、compact、limits 等 | 已有 7 类基础事件 | 服务型 hook 和部分事件缺失 |
| TUI | React/Ink 大量屏幕和组件 | Ratatui 版本，核心可用 | 产品化 UI、远程/团队/技能界面少 |
| Session | 本地 + 远程 + fork/tag/rename/list 等完整 API | 本地 JSONL + 基础恢复 | 远程同步、分支、标签等不足 |
| Sandbox | 平台级隔离与交互 UI | 配置/包装层为主 | 安全隔离不等价 |

---

## 六、建议开发优先级

### P0 — 兼容性与日常可用性

1. **统一或兼容文件工具 schema**
   - 决定继续使用 `path`，还是兼容原版 `file_path`。
   - 如果目标是 Claude Code 兼容，建议工具输入接受 `path` 与 `file_path`，输出/UI 内部统一一种表示。

2. **补齐原版任务工具族**
   - `TaskCreate`、`TaskGet`、`TaskList`、`TaskUpdate`、`TaskOutput`、`TaskStop`。
   - 这是 Agent Team、后台任务和长任务监控的基础。

3. **实现 `SendMessage`、`TeamCreate`、`TeamDelete`**
   - 支撑多代理协作。
   - 可先实现本地进程/会话内团队，后续再接 tmux/iTerm/remote backend。

4. **实现 `SkillTool` 与基础 skills 加载**
   - 先支持 `.claude/skills` / bundled skills 的最小 frontmatter + Markdown prompt。
   - 再接 slash suggestions 和工具调用。

5. **实现 `EnterWorktree` / `ExitWorktree`**
   - 提供隔离开发能力。
   - 与现有 session cwd、git context、permission/hooks 联动。

### P1 — 原版核心生态对齐

6. **MCP resources / auth / elicitation**
   - `ListMcpResources`、`ReadMcpResource`、`McpAuth`。
   - 支持 resources/prompts 浏览、MCP 协议层 OAuth/Auth、step-up auth、token revoke 和 elicitation UI。

7. **`stream-json` 输出协议** ✅ 已完成（迭代 46）
   - 第一版 NDJSON 事件流已落地（`message_start`/`content_block_delta`/`thinking_delta`/`tool_use`/`tool_result`/`usage`/`error`/`done`）。
   - 后续可继续对齐原版 SDK 协议细节（完整事件字段、附件、remote transport）。

8. **完善 `/resume` 交互式选择器**
   - 列表、搜索、预览、恢复指定 session。

9. **增强 Bash 安全语义**
    - destructive command warning、read-only validation、sed edit validation、git safety、sandbox decision。

10. **增强 FileRead/FileEdit/FileWrite**
    - 图片/PDF/notebook 读取、binary/device guard、相似路径提示、文件大小/token 限制、IDE/LSP 通知、settings 特殊保护。

### P2 — 产品体验与安全

11. **真正 OS-level sandbox**
    - macOS/Linux/Windows 分平台 adapter。
    - `/sandbox-toggle` 和 sandbox violation 展示。

12. **Permission UI 与规则引擎增强**
    - path-scoped rules、shadowed rule detection、规则解释、classifier decision。

13. **Plugin marketplace 与 policy**
    - marketplace browse/install/update/remove。
    - trust、validation、managed plugins。

14. **TUI 高级体验**
    - Vim 模式、output styles、voice、复杂 MCP/Agent/Team 面板。

15. **IDE 集成**
    - selection context、diff in IDE、IDE status indicator、LSP 插件推荐。

### P3 — 长期能力

16. **Remote sessions / Teleport（仅本地传输/会话层，不包含云端账号桥接）**
17. **Team/Swarm 多 backend：tmux、iTerm、remote**
18. **Voice/STT 完整集成**
19. **Bedrock / Vertex / Foundry provider 完整路由（仅服务端 API 配置，不包含账号 OAuth）**
20. **高级命令：`autofix-pr`、`ultraplan`、`security-review`、`thinkback`、`passes` 等**

---

## 七、旧结论修正记录

以下旧结论已不再准确：

| 旧结论 | 当前修正 |
|---|---|
| `WebSearch` 缺失或 dummy | Rust 已有 `WebSearchTool` 并作为 deferred tool 注册 |
| `AskUserQuestion` 缺失 | Rust 已有 `AskUserQuestionTool` 和 TUI user question 回调 |
| `Monitor` 缺失 | Rust 已有 `MonitorTool` |
| `EnterPlanMode` / `ExitPlanMode` 缺失 | Rust 已有对应工具 |
| TUI 仅最终文本，无 token streaming | Rust TUI bridge 已有 `StreamStart/StreamDelta/StreamEnd` 和 thinking delta |
| MCP 仅 stdio | Rust 已有 stdio/http/sse transport 基础实现 |
| Custom Agents 完全缺失 | Rust 已支持 `.claude/agents/*.md` 基础发现 |
| Plugin 系统完全缺失 | Rust 已有本地 plugin manifest 基础加载与注入 |
| Hooks 缺 SessionStart/SessionEnd | Rust hook 类型和 runner 已支持 SessionStart/SessionEnd |
| Agent SDK 完全缺失 | Rust 已有 `crates/sdk` 基础 Session API，但不等价于原版完整 Agent SDK |

---

## 八、当前最值得先做的校验项

如果下一步要继续缩小差异，建议先做以下源码级校验和实现任务：

1. 为文件工具增加 `file_path` 兼容输入，或明确文档化 Rust 使用 `path`。
2. 对照原版 task tools，设计 Rust 的共享任务存储和独立工具 schema。
3. 对照原版 `SkillTool` 和 `skills/loadSkillsDir.ts`，实现最小 skill loader。
4. 对照原版 worktree 工具，实现 `EnterWorktree` / `ExitWorktree`。
5. 对照原版 MCP resources/auth/elicitation，实现 `ListMcpResources`、`ReadMcpResource`、`McpAuth`。
6. ~~增加 `stream-json` output format，优先对齐原版 headless 事件流。~~ 已在迭代 46 完成第一版 NDJSON 输出。
