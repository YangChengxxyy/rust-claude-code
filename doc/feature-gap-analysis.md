# Rust Claude Code vs 原版 Claude Code 功能对比分析

> 生成时间: 2026-04-24
> 
> 本文档系统性对比 Rust 重写版与原版 TypeScript Claude Code 的功能差异，用于指导后续开发优先级。

---

## 一、已实现且基本对齐的功能

### 1.1 核心工具 (Tools)

| 工具 | 原版 | Rust版 | 状态 |
|------|------|--------|------|
| Bash | ✅ | ✅ | 对齐 |
| Read (FileRead) | ✅ | ✅ | 对齐 |
| Write (FileWrite) | ✅ | ✅ | 对齐 |
| Edit (FileEdit) | ✅ | ✅ | 对齐 |
| Glob | ✅ | ✅ | 对齐 |
| Grep | ✅ | ✅ | 对齐 |
| Agent (子代理) | ✅ | ✅ | 对齐 |
| TodoWrite | ✅ | ✅ | 对齐 |
| WebFetch | ✅ | ✅ | 对齐 |
| NotebookEdit | ✅ | ✅ | 对齐 |
| LSP | ✅ | ✅ | 对齐 |
| Task 管理 | ✅ | ✅ | 对齐 |

### 1.2 基础架构

| 功能 | 状态 | 备注 |
|------|------|------|
| Agentic Loop (多轮工具调用循环) | ✅ 对齐 | |
| SSE 流式响应 | ✅ 对齐 | |
| Prompt Caching | ✅ 对齐 | |
| 配置优先级链 (env > CLI > settings > config > default) | ✅ 对齐 | |
| 认证 (x-api-key / Bearer / apiKeyHelper) | ✅ 对齐 | |
| Session 持久化与恢复 (`--continue` / `--resume`) | ✅ 对齐 | |
| CLAUDE.md 读取 (项目/用户/全局层级) | ✅ 对齐 | |
| Memory Store (memory/ 目录) | ✅ 对齐 | |
| Git 上下文收集 (分支/最近提交) | ✅ 对齐 | |
| MCP (stdio 传输) | ✅ 对齐 | |
| Permission 5种模式 | ✅ 对齐 | Default / AcceptEdits / Bypass / Plan / DontAsk |
| Hooks (PreToolUse / PostToolUse / Stop 等) | ✅ 对齐 | |
| 对话压缩 (Compaction) | ✅ 对齐 | |
| Model alias (opus / sonnet / haiku) | ✅ 对齐 | |
| Thinking / Extended Thinking | ✅ 对齐 | |
| `--print` 非交互模式 | ✅ 对齐 | |
| JSON 输出格式 | ✅ 对齐 | |
| Max tokens 截断恢复 | ✅ 对齐 | 最多自动续接 3 次 |
| 工具并发执行模型 | ✅ 对齐 | concurrent-safe 并行，其余串行 |

---

## 二、缺失或部分实现的功能

### 2.1 缺失的工具

| 缺失工具 | 优先级 | 说明 |
|----------|--------|------|
| **WebSearch** | 高 | Rust版有框架但后端是 dummy，无实际搜索能力 |
| **AskUserQuestion** | 高 | 向用户提问的工具（多选题形式），交互式场景必需 |
| **Monitor** | 中 | 后台进程监控工具，原版可运行命令并实时回传输出 |
| **EnterPlanMode / ExitPlanMode** | 中 | 原版有专门的 plan mode 进入/退出工具，Rust版仅靠 `/mode` 切换 |
| **EnterWorktree / ExitWorktree** | 中 | Git worktree 隔离工具，用于并行任务 |
| **CronCreate / CronDelete / CronList** | 低 | 会话内定时任务调度 |
| **PowerShell** | 低 | Windows 原生命令执行 (仅 Windows 相关) |
| **SendMessage** | 低 | Agent Team 消息传递 |
| **TeamCreate / TeamDelete** | 低 | 多代理团队编排 |
| **Skill** | 低 | 技能加载执行工具 |
| **ToolSearch** | 低 | MCP 工具延迟加载/搜索 |
| **ListMcpResourcesTool / ReadMcpResourceTool** | 低 | MCP 资源浏览 |

### 2.2 缺失的 Slash 命令

Rust 版仅实现 **12 个** slash 命令，原版有 **60+** 个。

#### 高优先级

| 缺失命令 | 说明 |
|----------|------|
| `/init` | 初始化项目 CLAUDE.md |
| `/resume` (交互式) | 带列表选择的会话恢复 UI |
| `/context` | 上下文使用量可视化（彩色方格图） |
| `/permissions` | 交互式权限规则管理 |
| `/login` / `/logout` | Anthropic 账号登录登出 |
| `/status` | 展示详细状态信息 |

#### 中优先级

| 缺失命令 | 说明 |
|----------|------|
| `/doctor` | 安装和设置诊断 |
| `/review` | PR 代码审查 |
| `/batch` | 大规模并行代码变更编排 |
| `/voice` | 语音输入模式 |
| `/export` | 导出对话为文本文件 |
| `/copy` | 复制助手回复到剪贴板 |
| `/plan` | 通过命令进入 plan mode |
| `/rewind` / `/checkpoint` | 回退对话和代码 |
| `/add-dir` | 添加额外工作目录 |

#### 低优先级

| 缺失命令 | 说明 |
|----------|------|
| `/branch` | 对话分支 |
| `/rename` | 会话重命名 |
| `/theme` | 主题切换 |
| `/tui` | 渲染器切换（fullscreen 等） |
| `/focus` | 专注视图 |
| `/effort` | 模型努力等级 |
| `/autofix-pr` | CI PR 自动修复 |
| `/ultraplan` / `/ultrareview` | 深度多代理规划/审查 |
| `/schedule` | 定时例程 |
| `/loop` / `/proactive` | 重复执行 prompt |
| `/security-review` | 安全审查 |
| `/insights` | 使用分析报告 |
| `/recap` | 会话摘要 |

### 2.3 TUI 层面缺失

| 缺失功能 | 优先级 | 说明 |
|----------|--------|------|
| **实时 token 流式渲染** | 高 | Rust版有流式管线，但 TUI bridge 仅传最终文本，无逐 token 渲染 |
| **文件编辑 Diff 预览** | 高 | 原版审批 FileEdit 时显示 diff，Rust版没有 |
| **语法高亮代码块** | 中 | 原版有 language-aware 语法高亮，Rust版仅基础 markdown 渲染 |
| **多主题支持** | 中 | 原版有 light/dark/colorblind/ANSI/自定义主题 |
| **Fullscreen 渲染器** | 中 | 原版有 alt-screen 无闪烁渲染器 |
| **Focus 视图** | 低 | 只显示最后一轮问答 |
| **上下文使用量可视化** | 低 | 彩色方格图展示 context window 占用 |
| **交互式会话选择器** | 中 | resume 时列出历史会话供选择 |
| **Vim 编辑模式** | 低 | 原版支持 vim 键绑定 |

### 2.4 平台和服务集成缺失

| 缺失 | 优先级 | 说明 |
|------|--------|------|
| **Amazon Bedrock** | 中 | 原版支持 `CLAUDE_CODE_USE_BEDROCK` |
| **Google Vertex AI** | 中 | 原版支持 `CLAUDE_CODE_USE_VERTEX` |
| **Azure AI Foundry** | 低 | 原版支持 `CLAUDE_CODE_USE_FOUNDRY` |
| **MCP SSE/HTTP 传输** | 中 | Rust版仅支持 stdio，原版支持全部三种传输方式 |
| **MCP Channels** | 低 | 外部事件推送 |
| **MCP Elicitation** | 低 | 服务端交互式对话 |
| **IDE 集成** | 低 | VS Code / JetBrains / Cursor 插件 |
| **Desktop App** | 低 | 独立桌面应用 |
| **Web/Mobile** | 低 | claude.ai/code 和移动端 |
| **Remote Sessions** | 低 | `/remote-control` / `/teleport` |

### 2.5 Agent SDK (Headless 模式) — 完全缺失

| 缺失 | 说明 |
|------|------|
| TypeScript SDK | `@anthropic-ai/claude-agent-sdk` |
| Python SDK | `claude-agent-sdk` |
| 编程式 API 接口 | 原版可作为库嵌入其他应用 |

### 2.6 高级功能缺失

| 缺失 | 优先级 | 说明 |
|------|--------|------|
| **Plugin 系统** | 中 | 插件打包、分发、Marketplace |
| **Custom Agents** | 中 | `.claude/agents/` 自定义代理定义 |
| **Agent Teams** | 低 | 多代理协作编排 |
| **Sandbox 隔离** | 中 | OS 级文件系统/网络沙箱 |
| **Auto mode** | 中 | 带后台安全检查的自动审批模式 |
| **Managed settings** | 低 | 企业组织级策略强制执行 |
| **Path-scoped rules** | 中 | `.claude/rules/*.md` 带 `paths` frontmatter |
| **CLAUDE.local.md** | 中 | 个人项目级指令 (gitignore) |
| **@import 语法** | 低 | CLAUDE.md 中引用其他文件 |
| **Auto Memory** | 中 | 自动学习用户偏好并记忆 (无需手动操作) |
| **Git Worktree** | 低 | 并行任务隔离 |
| **Checkpointing / Rewind** | 低 | 对话与代码回退快照 |

---

## 三、存在实现差异的功能

| 方面 | 原版行为 | Rust版行为 | 影响 |
|------|----------|-----------|------|
| **默认 max_turns** | 无明确上限 (用户可中断) | 默认 8 轮 | 复杂任务可能提前停止 |
| **Bash 工作目录** | 持久化在 session 中，命令间保持 cd 状态 | 每次从 CWD 启动 | 连续 shell 操作体验差异 |
| **Permission 交互** | 实时弹窗 + diff 预览 | TUI 模态框 (基础，无 diff) | 用户审批信息量不足 |
| **Compaction 后恢复** | 重新注入 CLAUDE.md + 已调用 skills (前 5000 token/skill) | 仅基础摘要替换 | 原版上下文恢复更完善 |
| **Settings 层级** | 5 层 (managed / cli / local / project / user) | 3 层 (cli / project / user) | 缺 managed 和 local 层 |
| **Permission 规则语法** | 支持路径通配符 `Edit(/src/**/*.ts)` | 仅工具名 + 命令前缀 `Bash(git *)` | Rust版规则表达力较弱 |
| **Hook events** | 7 种 (含 SessionStart / SessionEnd / Elicitation) | 5 种 (缺 SessionStart / SessionEnd) | 钩子覆盖场景少 |
| **流式输出到 TUI** | 逐 token 实时渲染，含 thinking 折叠动画 | 仅最终文本一次性推送 | 交互体验差距明显 |

---

## 四、建议开发优先级

### P0 — 核心体验 (影响日常使用)

1. **TUI 实时 token 流式渲染** — 当前最大体验差距
2. **FileEdit Diff 预览** — 安全审批必需
3. **WebSearch 真实后端** — 工具完整性
4. **AskUserQuestion 工具** — 交互式场景必需
5. **`/init` 命令** — 新项目上手必需

### P1 — 功能完善 (提升可用性)

6. Bash 工作目录持久化
7. 交互式会话选择器 (`/resume`)
8. `/context` 上下文可视化
9. `/permissions` 交互管理
10. Permission 规则路径通配符语法
11. MCP SSE/HTTP 传输支持
12. `/login` / `/logout`
13. Monitor 工具
14. Auto Memory (自动学习偏好)

### P2 — 生态扩展 (提升竞争力)

15. Bedrock / Vertex AI 支持
16. Plugin 系统
17. Custom Agents (`.claude/agents/`)
18. Sandbox 隔离
19. Auto mode
20. Agent SDK (Rust crate 形式)
21. 语法高亮代码块
22. 多主题支持

### P3 — 长期演进

23. Agent Teams
24. Git Worktree 工具
25. Checkpointing / Rewind
26. IDE 集成
27. Remote Sessions
28. `/batch` / `/ultraplan` / `/ultrareview`
