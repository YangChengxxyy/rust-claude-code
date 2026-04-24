# 第三期迭代计划 — 从"可用"到"好用"

> 生成时间: 2026-04-24
>
> 基于 `doc/feature-gap-analysis.md` 的差距分析，制定从迭代 23 起的第三期规划。
>
> 第三期核心目标：**消除日常使用中的体验断层，补齐高频缺失能力，建立生态扩展基础。**

---

## 0. 当前状态概览

第一期（迭代 1-11）：基础骨架，从零到可运行。
第二期（迭代 12-22）：核心工具补齐、MCP、Agent、Hooks、Memory，达到"能日常使用"。

**第三期起点**：12 个核心工具已对齐，架构基本完整，但存在以下三类断层：

| 断层类型 | 典型表现 | 影响 |
|----------|----------|------|
| **体验断层** | TUI 无逐 token 流式渲染、FileEdit 无 diff 预览、无语法高亮 | 每次交互都能感知 |
| **能力断层** | 无 WebSearch 真实后端、无 AskUserQuestion、Bash 不持久化 CWD | 限制 agent 能力上限 |
| **生态断层** | 无 Bedrock/Vertex、MCP 仅 stdio、无 Plugin/Custom Agent | 限制适用场景 |

---

## 1. 第三期分层策略

按"用户感知价值 × 实现复杂度"分为 4 个阶段：

```
阶段 A（迭代 23-25）: 体验打磨     — 让每次交互都更好
阶段 B（迭代 26-28）: 能力补齐     — 让 agent 能做更多事
阶段 C（迭代 29-31）: 工程化增强   — 让项目管理更完善
阶段 D（迭代 32-34）: 生态扩展     — 让更多场景可接入
```

---

## 2. 阶段 A：体验打磨（迭代 23-25）

> 目标：消除日常使用中每次交互都能感知到的体验差距。

### 迭代 23：TUI 流式渲染全链路

**状态**: 规划中

**问题**: 当前 TUI bridge 虽有 `StreamDelta` 事件，但流式渲染在实际使用中仍有卡顿、闪烁、不完整等问题。这是与原版最大的体验差距。

**目标**: 实现稳定、流畅的逐 token 流式渲染，包括文本、thinking、工具调用全阶段。

**产出**:

- `tui` crate:
  - 流式文本渲染优化：解决闪烁和重绘效率问题
  - Thinking 阶段流式渲染：实时显示思考过程，带折叠/展开动画
  - 工具调用流式显示：tool_use 的 JSON 参数逐步出现
  - 流式 Markdown 增量解析：不等待完整段落，逐行渲染
  - 光标位置和自动滚动逻辑优化
- `cli` crate:
  - 确保 `--print` 模式也有流畅的逐 token 输出
  - 流式输出中断（Ctrl+C / Esc）的清理逻辑加固

**验收标准**:

- 长回复（>500 token）过程中无明显闪烁
- Thinking 内容实时可见且可折叠
- 工具调用参数逐步展现
- Ctrl+C 中断后终端状态干净
- 性能：渲染不应引入 >50ms 的单帧延迟

**依赖**: 无

---

### 迭代 24：FileEdit Diff 预览 + 语法高亮

**状态**: 规划中

**问题**: FileEdit 审批时用户无法看到具体变更内容，只能看到工具名和参数摘要。原版会显示完整的 diff 视图。代码块无语法高亮，可读性差。

**目标**: 在权限审批和工具结果展示中提供 diff 预览；代码块增加语法高亮。

**产出**:

- `tui` crate:
  - 权限对话框增强：FileEdit/FileWrite 审批时显示 unified diff 预览
  - Diff 渲染组件：红/绿着色、行号、上下文行
  - 代码块语法高亮：集成 `syntect` 或 `tree-sitter-highlight`
  - 支持常见语言：Rust, Python, TypeScript/JavaScript, Go, Java, JSON, YAML, Markdown, Shell
  - Inline code 和代码块统一高亮风格
- `tools` crate:
  - FileEditTool 在结果中返回 diff 信息（old/new 内容片段）
  - FileWriteTool 在结果中标注是创建还是覆盖

**验收标准**:

- FileEdit 审批弹窗中能看到红/绿 diff
- 代码块中 Rust/Python/TypeScript 有正确的关键字/字符串/注释着色
- 语法高亮不引入明显性能下降（<100ms 渲染延迟）
- 不支持的语言 graceful fallback 为纯色

**依赖**: 无（可与迭代 23 并行）

---

### 迭代 25：TUI 进阶交互

**状态**: 规划中

**问题**: 缺少多种提升日常效率的交互功能 — 会话选择器、上下文可视化、主题切换。

**目标**: 补齐高频使用的 TUI 交互能力。

**产出**:

- `tui` crate:
  - 交互式会话选择器（`/resume` 增强）：列出最近 N 个会话，显示时间/模型/首条消息摘要，方向键选择
  - `/context` 命令：彩色方格图展示 context window 占用比例（system prompt / 消息 / 工具结果 / 剩余空间）
  - `/export` 命令：导出当前对话为 Markdown 文件
  - `/copy` 命令：复制最近一条助手回复到系统剪贴板
  - 多主题支持：dark（默认）/ light / 自定义主题文件（`~/.config/rust-claude-code/theme.json`）
  - `/theme` 命令：切换主题
- `cli` crate:
  - Session 列表查询接口（供交互式选择器使用）
  - Session 元数据增强：首条消息摘要、消息数量

**验收标准**:

- `/resume` 弹出会话列表，选择后正确恢复
- `/context` 显示可读的上下文占用图
- `/export` 输出合法的 Markdown 文件
- 主题切换后即时生效
- 相关测试通过

**依赖**: 迭代 23（依赖流式渲染稳定）

---

## 3. 阶段 B：能力补齐（迭代 26-28）

> 目标：消除限制 agent 能力上限的功能缺口。

### 迭代 26：Bash 持久化 + AskUserQuestion + WebSearch

**状态**: 规划中

**问题**: 三个独立但都高频影响 agent 能力的缺口。Bash 工作目录不持久化导致 `cd` 无效；缺少 AskUserQuestion 导致 agent 无法向用户确认选项；WebSearch 无真实后端。

**目标**: 补齐三个高影响力工具能力。

**产出**:

- `tools` crate — Bash 工作目录持久化:
  - 维护每个 session 的当前工作目录（`session_cwd`）
  - `cd` 命令执行后更新 `session_cwd`
  - 后续命令自动使用更新后的 `session_cwd`
  - 安全边界：限制在项目根目录范围内
- `tools` crate — `AskUserQuestionTool`:
  - Input: `question`、`options`（数组，每项含 label + description）、`allow_custom`（是否允许自由输入）
  - 通过 TuiBridge 向 TUI 发起交互请求（复用权限对话框模式的 oneshot channel）
  - TUI 中渲染选项列表，用户选择后返回结果
  - `--print` 模式下自动选择第一个选项（或使用 stdin）
- `tools` crate — WebSearch 真实后端:
  - 实现 `SearchBackend` trait 的至少一个真实后端
  - 方案优先级：Brave Search API > Tavily API > SearXNG 自托管
  - 配置通过环境变量或 settings.json（`webSearchProvider`、`webSearchApiKey`）
  - 结果格式化：标题、URL、摘要片段
  - 域名过滤（allow/block list）已有框架，接入真实后端

**验收标准**:

- `cd /tmp && pwd` 返回 `/tmp`，后续 `ls` 在 `/tmp` 下执行
- AskUserQuestion 在 TUI 中弹出选项，用户选择后 agent 收到结果
- WebSearch 能返回真实搜索结果（至少一个后端可用）
- 相关测试通过

**依赖**: 无

---

### 迭代 27：Permission 增强 + Settings 完善

**状态**: 规划中

**问题**: Permission 规则只支持工具名 + 命令前缀，不支持路径通配符；Settings 缺少 local 层和 managed 层；缺少 `/permissions` 交互管理。

**目标**: 提升权限系统的表达力，完善配置分层。

**产出**:

- `core` crate — Permission 规则增强:
  - 支持路径通配符语法：`Edit(/src/**/*.ts)`、`Read(./.env)`
  - 支持 `//path`（绝对路径）、`~/path`（home 相对）、`/path`（项目根相对）、`./path`（CWD 相对）
  - 规则评估扩展为三级：Deny > Ask > Allow（新增 Ask 级别）
  - FileEdit/FileWrite/FileRead 自动提取文件路径参与规则匹配
- `core` crate — Settings 分层补齐:
  - 支持 `.claude/settings.local.json`（不入版本控制，优先级高于 `.claude/settings.json`）
  - 支持 `CLAUDE.local.md`（个人项目指令，gitignore）
  - 支持 `.claude/rules/*.md`（路径范围规则文件，带 `paths` frontmatter）
  - Settings 合并优先级更新为：CLI > env > local project > shared project > user > default
- `tui` crate:
  - `/permissions` 命令：显示当前所有规则（来源标注），支持添加/删除
  - `/init` 命令：在项目根目录创建 `.claude/` 目录和基础 `CLAUDE.md`
  - `/status` 命令：综合展示模型、权限、MCP、hooks、memory 状态

**验收标准**:

- `Edit(/src/**/*.ts)` 规则正确匹配 TypeScript 文件编辑
- `.claude/settings.local.json` 覆盖 `.claude/settings.json` 的同名字段
- `CLAUDE.local.md` 内容出现在 system prompt 中
- `/permissions` 可查看和管理规则
- `/init` 正确创建项目结构
- 相关测试通过

**依赖**: 无（可与迭代 26 并行）

---

### 迭代 28：Monitor 工具 + PlanMode 工具 + Compaction 增强

**状态**: 规划中

**问题**: 缺少 Monitor 后台进程监控工具；Plan mode 只能通过 `/mode` 切换，缺少 EnterPlanMode/ExitPlanMode 工具让 agent 自主切换；Compaction 后缺少 CLAUDE.md 重新注入。

**目标**: 补齐影响复杂任务场景的工具和机制。

**产出**:

- `tools` crate — `MonitorTool`:
  - Input: `command`、`pattern`（正则匹配感兴趣的输出行）、`timeout`
  - 在后台 spawn 进程，实时读取 stdout/stderr
  - 匹配的输出行作为事件推送给 agent
  - 适用场景：`cargo watch`、`npm run dev`、`docker logs -f`
  - 进程生命周期管理：超时自动终止、手动终止接口
- `tools` crate — `EnterPlanModeTool` / `ExitPlanModeTool`:
  - EnterPlanMode：切换到 Plan mode，只允许只读操作
  - ExitPlanMode：提交计划摘要，恢复原权限模式
  - Agent 可自主决定何时进入规划模式
- `cli` crate — Compaction 增强:
  - Compaction 后自动重新注入项目根 CLAUDE.md
  - Compaction 后重新注入已调用的 MCP server 工具列表摘要
  - Compaction 后保留最近的 permission 决策上下文
  - `/compact` 支持可选参数指定保留策略

**验收标准**:

- MonitorTool 能在 `cargo test` 运行时实时回传测试输出
- Agent 能自主 enter/exit plan mode
- Compaction 后 CLAUDE.md 内容仍在 system prompt 中
- 相关测试通过

**依赖**: 迭代 26（Monitor 依赖 Bash 持久化 CWD 相关基础）

---

## 4. 阶段 C：工程化增强（迭代 29-31）

> 目标：增强在真实工程项目中的协作和管理能力。

### 迭代 29：Auto Memory + /doctor + /review

**状态**: 规划中

**问题**: 当前 Memory 系统需要用户手动 `/memory remember`，原版支持自动记忆；缺少诊断工具和 PR 审查能力。

**目标**: 让记忆系统更智能，增加工程协作命令。

**产出**:

- `core` / `cli`:
  - Auto Memory：agent 在 system prompt 指引下自动写入记忆
  - 触发条件：用户纠正错误、明确表达偏好、项目上下文变更
  - 记忆去重：新记忆与现有记忆相似度检查
  - `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` 禁用开关
- `tui` crate:
  - `/doctor` 命令：检查环境配置
    - API 连通性检查
    - 配置文件解析验证
    - MCP 服务器状态检查
    - 工具可用性检查
    - 权限文件完整性检查
    - 输出诊断报告
  - `/review` 命令：PR 代码审查
    - Input: PR 号码或 URL（可选，默认检测当前分支）
    - 自动运行 `git diff` 获取变更
    - 构造审查 prompt 让 agent 分析代码
    - 输出结构化的审查意见

**验收标准**:

- 用户纠正 agent 后，相关偏好自动写入 memory
- `/doctor` 输出完整的环境诊断报告
- `/review` 能对当前分支 diff 给出代码审查意见
- 相关测试通过

**依赖**: 迭代 27（依赖 Settings 完善）

---

### 迭代 30：Slash Commands 大批量补齐

**状态**: 规划中

**问题**: Rust 版仅 12 个 slash 命令，原版 60+。许多命令实现简单但提升日常效率。

**目标**: 批量补齐中等优先级的 slash 命令。

**产出**:

- `/plan [description]` — 快速进入 plan mode 并附带上下文
- `/rename [name]` — 重命名当前会话
- `/branch [name]` — 创建对话分支（fork 当前消息历史）
- `/recap` — 生成当前会话的摘要
- `/rewind` — 回退到上一个用户消息（撤销最近一轮）
- `/add-dir <path>` — 添加额外工作目录
- `/login` / `/logout` — Anthropic 账号管理（基础，对接 `apiKeyHelper`）
- `/effort [level]` — 设置模型努力等级（low/medium/high → 调整 thinking budget）
- `/keybindings` — 显示所有快捷键
- 命令注册框架重构：从静态数组迁移到动态注册表，支持命令自描述、参数补全

**验收标准**:

- 所有新命令在 TUI 中可用
- `/help` 自动列出所有已注册命令
- `/rewind` 正确回退消息历史
- `/branch` 创建独立的消息历史分支
- 命令注册表支持动态注册
- 相关测试通过

**依赖**: 迭代 25（依赖 TUI 进阶交互基础）

---

### 迭代 31：Hook 增强 + Custom Agents

**状态**: 规划中

**问题**: Hook 仅支持 command 类型，缺少 SessionStart/SessionEnd 事件；不支持 Custom Agents。

**目标**: 增强 Hook 覆盖范围，引入 Custom Agent 定义。

**产出**:

- `core` / `cli` — Hook 增强:
  - 新增 `SessionStart` 事件：会话开始时触发（可用于环境初始化）
  - 新增 `SessionEnd` 事件：会话结束时触发（可用于清理/日志）
  - Hook 结果增强：支持 `updatedInput`（修改工具输入参数）
  - Hook 执行增强：支持 `once` 标志（只触发一次）
- `core` / `cli` — Custom Agents:
  - `.claude/agents/` 目录发现
  - Agent 定义文件格式：name、description、system_prompt、tools（白名单）、model（可选）
  - Agent 注册到工具系统，可被主 agent 通过 AgentTool 调用
  - `/agents` 命令：列出可用的自定义 agent
- `tools` crate:
  - AgentTool 增强：支持指定 custom agent 名称

**验收标准**:

- SessionStart hook 在会话启动时触发
- Custom agent 定义文件被正确加载
- 主 agent 能通过 AgentTool 调用 custom agent
- `/agents` 列出所有自定义 agent
- 相关测试通过

**依赖**: 迭代 29（依赖 Auto Memory 和 doctor 基础设施）

---

## 5. 阶段 D：生态扩展（迭代 32-34）

> 目标：让 Rust 版本能接入更广泛的场景。

### 迭代 32：MCP SSE/HTTP + Bedrock/Vertex

**状态**: 规划中

**问题**: MCP 仅支持 stdio 传输，无法接入远程 MCP 服务器；不支持 Bedrock/Vertex 等云平台 provider。

**目标**: 扩展连接能力，支持更多 MCP 传输和 LLM provider。

**产出**:

- `mcp` crate — 传输扩展:
  - SSE (Server-Sent Events) 传输实现
  - HTTP (Streamable HTTP) 传输实现
  - 传输类型自动选择（基于配置的 `type` 字段）
  - 自动重连逻辑（指数退避）
- `api` crate — Provider 扩展:
  - `ProviderConfig` 枚举：Anthropic / Bedrock / Vertex
  - Amazon Bedrock：`CLAUDE_CODE_USE_BEDROCK=1` + AWS credentials
  - Google Vertex AI：`CLAUDE_CODE_USE_VERTEX=1` + GCP credentials
  - Provider-specific 请求签名和认证
  - Provider-specific 端点构造
  - `--provider` CLI 参数 或环境变量选择
- `core` crate:
  - Provider 配置在 Settings 中声明

**验收标准**:

- 能连接远程 SSE MCP 服务器并调用工具
- 能通过 Bedrock 发送请求并正常对话
- 能通过 Vertex AI 发送请求并正常对话
- Provider 切换不影响工具系统和权限系统
- 相关测试通过

**依赖**: 无（可独立推进）

---

### 迭代 33：Sandbox 隔离 + Auto Mode

**状态**: 规划中

**问题**: 缺少运行时隔离机制，在不可信环境中使用有安全风险；缺少 Auto mode（自动审批+后台安全检查）。

**目标**: 建立安全运行边界。

**产出**:

- `core` / `cli` — Sandbox:
  - macOS：使用 `sandbox-exec` 限制文件系统访问范围
  - Linux：使用 `bubblewrap` 或 namespace 隔离
  - 网络限制：可选禁止出站网络（`--sandbox-no-network`）
  - 配置：`sandbox.enabled`、`sandbox.allowed_paths`、`sandbox.network`
  - `autoAllowBashIfSandboxed`：sandbox 内 Bash 跳过权限确认
- `core` / `cli` — Auto Mode:
  - 新增 `PermissionMode::Auto`
  - 自动审批工具调用，但在后台执行安全检查
  - 安全检查：路径范围验证、危险命令检测、输出异常检测
  - 安全检查失败时回退到交互确认
  - `--mode auto` CLI 参数

**验收标准**:

- Sandbox 模式下 Bash 无法访问项目目录外的文件
- Auto mode 下常规操作自动通过，危险操作仍需确认
- Sandbox + Auto mode 组合可正常工作
- 相关测试通过

**依赖**: 迭代 27（依赖 Permission 增强）

---

### 迭代 34：Plugin 系统 + SDK 基础

**状态**: 规划中

**问题**: 无法通过插件扩展功能；无法作为库被其他程序嵌入。

**目标**: 建立扩展和嵌入的基础。

**产出**:

- Plugin 系统:
  - Plugin manifest 格式定义（`plugin.json`）
  - Plugin 可提供：slash commands、MCP servers、custom agents、工具
  - Plugin 发现：`~/.claude/plugins/` 目录
  - Plugin 安装：`/plugin install <name/path>`
  - Plugin 加载：启动时自动加载已安装插件
  - `/plugin list` / `/plugin install` / `/plugin remove`
  - `/reload-plugins` 热重载
- SDK 基础（Rust crate）:
  - 将 `cli` crate 中的 QueryLoop 抽取为可嵌入的 `rust-claude-sdk` crate
  - 公开 API：`Session::new()` → `session.send(prompt)` → `Stream<Response>`
  - 支持自定义工具注入
  - 支持 Hook 回调
  - 非交互模式（headless）完整支持
  - 示例：`examples/sdk_basic.rs`

**验收标准**:

- 能安装并加载一个示例插件
- Plugin 提供的 slash command 可用
- SDK crate 可独立编译
- 示例程序能通过 SDK 发起对话并执行工具
- 相关测试通过

**依赖**: 迭代 31（依赖 Custom Agent 和 Hook 增强）

---

## 6. 依赖关系图

```
阶段 A（体验打磨）
┌─────────────────────────────────────────────────┐
│                                                 │
│  迭代 23 (流式渲染) ──┬── 迭代 25 (TUI 进阶)   │
│                       │                         │
│  迭代 24 (Diff+高亮) ─┘                         │
│        [23 和 24 可并行]                         │
└─────────────────────────────────────────────────┘
          │
阶段 B（能力补齐）
┌─────────────────────────────────────────────────┐
│                                                 │
│  迭代 26 (Bash+Ask+Search) ── 迭代 28 (Monitor  │
│                                + Plan + Compact)│
│  迭代 27 (Perm+Settings)                        │
│        [26 和 27 可并行]                         │
└─────────────────────────────────────────────────┘
          │
阶段 C（工程化增强）
┌─────────────────────────────────────────────────┐
│                                                 │
│  迭代 29 (AutoMem+Doctor+Review)                │
│        │                                        │
│  迭代 30 (Slash Commands 批量)                   │
│        │                                        │
│  迭代 31 (Hook增强+Custom Agents)               │
│                                                 │
└─────────────────────────────────────────────────┘
          │
阶段 D（生态扩展）
┌─────────────────────────────────────────────────┐
│                                                 │
│  迭代 32 (MCP SSE/HTTP + Bedrock/Vertex)        │
│        [可独立推进，不依赖阶段 C]                 │
│                                                 │
│  迭代 33 (Sandbox + Auto Mode)                  │
│        [依赖迭代 27]                             │
│                                                 │
│  迭代 34 (Plugin + SDK)                         │
│        [依赖迭代 31]                             │
│                                                 │
└─────────────────────────────────────────────────┘
```

**可并行推进的组合**:

- 迭代 23 + 迭代 24（流式渲染与 Diff/高亮互不依赖）
- 迭代 26 + 迭代 27（工具能力与权限系统互不依赖）
- 迭代 32 可在阶段 B 完成后独立推进（不依赖阶段 C）
- 迭代 33 在迭代 27 完成后即可推进（不依赖阶段 C 其余部分）

---

## 7. 阶段完成判定

### 阶段 A 完成标准

- [ ] 长回复流式渲染流畅无闪烁
- [ ] FileEdit 审批有 diff 预览
- [ ] 代码块有语法高亮
- [ ] 会话选择器、上下文可视化、导出功能可用
- [ ] 日常使用体验不再有"明显比原版差"的感知

### 阶段 B 完成标准

- [ ] Bash `cd` 在后续命令中生效
- [ ] AskUserQuestion 工具可用
- [ ] WebSearch 返回真实搜索结果
- [ ] Permission 规则支持路径通配符
- [ ] `.claude/settings.local.json` 和 `CLAUDE.local.md` 可用
- [ ] Monitor 工具可用
- [ ] Agent 可自主 enter/exit plan mode
- [ ] Compaction 后 CLAUDE.md 自动恢复

### 阶段 C 完成标准

- [ ] Auto Memory 按预期自动保存偏好
- [ ] `/doctor` 输出诊断报告
- [ ] `/review` 可审查代码
- [ ] Slash commands 达到 25+ 个
- [ ] Custom Agents 可定义和调用
- [ ] Hook 事件覆盖完整的生命周期

### 阶段 D 完成标准

- [ ] 能连接远程 MCP 服务器（SSE/HTTP）
- [ ] 能通过 Bedrock 或 Vertex AI 发送请求
- [ ] Sandbox 隔离可正常工作
- [ ] Auto mode 可正常工作
- [ ] Plugin 可安装和加载
- [ ] SDK crate 可独立编译和使用

---

## 8. 不在第三期范围的内容

以下功能作为长期方向记录，不纳入第三期交付：

- Agent Teams（多代理协作编排）— 等 Custom Agent 稳定后再推进
- Git Worktree 工具 — 使用频率低，可用 Bash 替代
- IDE 集成（VS Code / JetBrains 插件）— 需要独立项目
- Desktop App / Web / Mobile — 超出 CLI 工具定位
- Remote Sessions — 需要独立基础设施
- 企业级 Managed Settings（MDM / policy）— 需求不明确
- `/batch` / `/ultraplan` / `/ultrareview` — 依赖 Agent Teams
- 完整插件 Marketplace — Plugin 系统稳定后再考虑
- Voice 输入 — 依赖平台 API，实现复杂

---

## 9. 工作量估算

| 迭代 | 预估工作量 | 核心改动 crate |
|------|------------|----------------|
| 23 (流式渲染) | 中 | tui, cli |
| 24 (Diff+高亮) | 中 | tui, tools |
| 25 (TUI 进阶) | 中 | tui, cli |
| 26 (Bash+Ask+Search) | 大 | tools, tui |
| 27 (Perm+Settings) | 大 | core, tui |
| 28 (Monitor+Plan+Compact) | 中 | tools, cli |
| 29 (AutoMem+Doctor+Review) | 中 | core, cli, tui |
| 30 (Slash Commands) | 小 | tui, cli |
| 31 (Hook+Custom Agent) | 中 | core, cli, tools |
| 32 (MCP+Provider) | 大 | mcp, api |
| 33 (Sandbox+Auto) | 中 | core, cli |
| 34 (Plugin+SDK) | 大 | 新 crate + cli |
