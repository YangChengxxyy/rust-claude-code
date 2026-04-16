## Context

当前仓库已经具备用户级 `~/.claude/settings.json` 读取、CLI 参数覆盖、环境变量覆盖、CLAUDE.md 发现、system prompt 组合、基础 slash commands 与 TUI 状态栏，但这些能力仍各自演进：配置链路主要围绕用户级 settings 和进程环境变量，尚未正式纳入项目级 `.claude/settings.json`；Git 上下文仍只停留在外部环境说明，没有统一的运行时数据结构进入 CLI/TUI；slash commands 已有基础命令，但缺少与当前工作区状态直接相关的 `/diff`、`/cost`、`/config`。

本轮变更跨越 `core`、`cli`、`tui` 三个 crate，并且会修改 CLAUDE.md/Git root 发现边界，因此需要先明确几个关键技术决策：配置来源如何表达与合并、Git 快照如何采集与注入、slash command 如何从零散匹配收敛为可扩展注册入口，以及 `.claude/settings.json` 与 CLAUDE.md 是否共享同一套项目根定位逻辑。

## Goals / Non-Goals

**Goals:**
- 支持项目级 `.claude/settings.json`，并明确 CLI、env、项目级 settings、用户级 settings 的合并顺序
- 让运行时能够保留“配置值 + 来源”，为 `/config` 输出和调试提供基础
- 将 Git root、branch、clean 状态、recent commits 收敛为可复用上下文，并注入 system prompt 与 TUI 状态栏
- 为 `/diff`、`/cost`、`/config` 与增强版 `/clear` 定义统一的命令注册和执行边界
- 让 CLAUDE.md 发现与项目级 settings 发现共享 Git root 归一化逻辑，避免路径边界分裂

**Non-Goals:**
- 不在本轮引入完整 JSON Schema 校验引擎或外部 schema 发布流程，只实现本地结构校验与友好错误
- 不在本轮实现 Git diff 的交互式分页、彩色语法高亮或 staged/unstaged 复杂过滤 UI
- 不重构 QueryLoop 主循环架构，只补齐配置、上下文注入与命令分发所需的最小接口
- 不实现动态 slash commands、skills 或 MCP 命令注册，只为后续留出统一扩展点
- 不把 Git 上下文做成持续后台 watcher，本轮只做按需采集与刷新

## Decisions

### D1: 引入带来源信息的配置解析结果，而不是只返回最终 Config

**选择**: 在 `core` 中把 settings/config 的解析结果扩展为“值 + 来源”模型。最终运行时仍可投影为现有 `Config` / `SessionSettings`，但内部保留每个关键字段来自 CLI、env、project settings、user settings 或默认值的信息。

**理由**: `/config` 需要展示当前值来自哪里；仅保留最终值会迫使 CLI 侧重复实现一遍优先级解释逻辑，也不利于测试。

**替代方案**: 只在 CLI 中拼一份临时来源描述 —— 拒绝，因为这会让来源链路分散在多个调用点，后续扩展 permissions、model、base_url 时容易失真。

### D2: 项目级 `.claude/settings.json` 与 CLAUDE.md 共用 Git root 发现边界

**选择**: 项目级配置发现从当前工作目录向上遍历，到 git repository root 为止；`.claude/settings.json` 与 `CLAUDE.md` 共享同一套 git root 检测与 canonical path 归一化逻辑。

**理由**: 两者都表达“项目范围内的本地指令/配置”，共享边界能减少“配置在 repo 外生效但 CLAUDE.md 不生效”这类不一致。

**替代方案**: 项目 settings 直接只查 cwd 下 `.claude/settings.json` —— 拒绝，因为嵌套子目录启动时会丢失项目根配置，与用户预期不符。

### D3: Git 上下文采用一次性快照对象，在需要时刷新

**选择**: 在 `core` 或 `cli` 中定义 `GitContextSnapshot`，字段至少包含 `repo_root`、`branch`、`is_clean`、`recent_commits`。CLI 在构建 system prompt、进入 TUI、执行相关命令时按需刷新该快照；TUI 状态栏消费最近一次快照。

**理由**: Git 信息主要用于展示和 prompt 注入，不需要持续订阅。快照模型简单、可测试，也方便 `/config` / `/diff` / 状态栏复用。

**替代方案**: 在 TUI 内部独立执行 git 命令实时刷新 —— 拒绝，因为会复制采集逻辑，并把 Git 依赖散落到 UI 层。

### D4: slash commands 收敛为注册表，而不是继续在输入处理里做字符串分支

**选择**: 把 slash commands 抽象成统一命令注册结构，至少包含命令名、帮助文本、执行入口和是否需要 worker/QueryLoop 上下文。现有 `/clear`、`/compact`、`/mode`、`/model`、`/todo`、`/help`、`/exit` 迁移到同一框架，本轮新增 `/diff`、`/cost`、`/config`。

**理由**: 新命令已经开始依赖 Git、usage、配置来源等不同上下文；继续在 `app.rs` 中使用硬编码匹配会让逻辑迅速发散。

**替代方案**: 保持现状，按命令逐个追加 `match` 分支 —— 拒绝，因为后续技能、MCP 命令很快会让维护成本失控。

### D5: `/clear` 增强为显式模式，而不是隐式改变默认语义

**选择**: 保持现有 `/clear` 的默认清空行为，同时允许通过参数（如 `keep-context` 等语义）只清理显示消息或输入状态但保留底层上下文摘要/会话设置，具体命令解析由统一注册表负责。

**理由**: 直接改变 `/clear` 既有语义会破坏当前用户心智；显式模式更容易测试，也更符合命令兼容性。

**替代方案**: 直接把 `/clear` 改成“默认保留上下文” —— 拒绝，因为这属于破坏性行为变化，且没有必要。

### D6: Git diff 与 recent commits 通过受控命令调用采集，并在非 Git 目录优雅降级

**选择**: 统一通过受控 Git 命令获取 branch、status、recent commits、diff 摘要；若当前目录不在 git repo 中，则返回空 Git 上下文，相关 UI/命令给出无仓库提示而非报错退出。

**理由**: 该仓库已经依赖 git root 语义，命令行采集是最直接且实现成本最低的路径。优雅降级可以保证 CLI/TUI 在非 Git 目录仍可运行。

**替代方案**: 引入 `git2` 等库直接读 repository —— 暂不采用，因为本轮重点是行为收敛，不需要额外依赖与 FFI 复杂度。

## Risks / Trade-offs

- **[配置优先级回归]** → 用表驱动测试覆盖 CLI/env/project/user/default 的组合，确保现有 `RUST_CLAUDE_*` 与 `ANTHROPIC_*` 优先级不被破坏。
- **[Git 命令注入或路径歧义]** → 仅执行固定 Git 子命令，不拼接用户输入；对 repo root 和 cwd 做 canonicalize 归一化。
- **[slash command 重构影响现有命令]** → 先迁移现有命令行为，再增量添加新命令，并为 `/help` 输出做快照测试。
- **[项目级 settings 与用户级 settings 合并结果难以理解]** → 保留字段来源信息，并通过 `/config` 明确展示最终值及来源链。
- **[非 Git 目录状态栏或 system prompt 出现噪音]** → 约束无 Git 上下文时不注入冗余占位文本，只在相关命令调用时返回简洁提示。

## Migration Plan

- 无需数据迁移；新能力以向后兼容方式引入
- 现有用户级 `~/.claude/settings.json` 保持可用，项目级 `.claude/settings.json` 仅在存在时参与合并
- 现有 slash commands 名称保持不变，新命令增量加入 `/help`
- 如 Git 上下文采集失败，降级为空上下文，不阻断主流程

## Open Questions

- `/diff` 第一版是否需要显式区分 staged/unstaged 参数，还是先输出统一工作树 diff
- `/cost` 的成本估算是否只基于当前会话 usage 与模型单价占位文案，还是需要引入可配置价格表
- 项目级 settings 的校验错误应当阻止启动，还是以警告方式降级忽略相关字段
