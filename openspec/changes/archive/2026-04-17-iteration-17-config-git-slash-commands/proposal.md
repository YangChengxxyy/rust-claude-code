## Why

当前仓库已经具备基础的 QueryLoop、TUI、session 与 slash commands，但配置来源仍分散，Git 上下文还没有进入统一运行时视图，slash commands 也缺少与当前会话状态强相关的常用能力。这导致项目级配置难以与用户级配置协同，TUI 无法直观看到分支状态，system prompt 也无法稳定携带工作树快照，因此需要把下一轮重点放在配置收敛、Git 感知与命令体系补齐上。

## What Changes

- 收敛配置加载链路，补齐项目级 `.claude/settings.json` 支持，并明确 CLI、环境变量、项目配置、用户配置之间的优先级合并规则
- 为 settings 增加 `permissions` 字段解析与校验入口，使权限规则可以从配置统一进入运行时
- 增加配置来源可观测性，让 CLI/TUI 能展示当前生效配置及其来源
- 引入 Git 项目上下文采集，包括 git root、当前 branch、working tree 是否 clean、最近 commits 摘要，并注入 TUI 状态与 system prompt
- 扩展 slash commands，增加 `/diff`、`/cost`、`/config`，并增强 `/clear` 的清理语义与命令注册边界

## Capabilities

### New Capabilities
- `settings-merge`: 合并 CLI、env、项目级 settings、用户级 settings，并暴露配置来源信息
- `git-context-integration`: 采集 Git 工作区上下文并注入 system prompt 与 TUI 状态栏
- `slash-command-extensions`: 提供 `/diff`、`/cost`、`/config` 与增强版 `/clear` 的用户可见行为

### Modified Capabilities
- `claude-md-discovery`: Git root 检测参与项目级 `.claude/settings.json` 与 CLAUDE.md 的路径归一化与发现边界

## Impact

- **`crates/core`**: 扩展 settings/config 类型、来源跟踪、permissions 配置解析、Git 上下文类型
- **`crates/cli`**: 收敛配置优先级链，补齐 git status 快照采集、system prompt 注入、slash command 注册与执行
- **`crates/tui`**: 状态栏展示 branch 信息，承接 `/diff`、`/cost`、`/config`、增强版 `/clear` 的交互输出
- **配置文件**: 新增项目级 `.claude/settings.json` 兼容读取与 schema 校验路径
- **依赖/系统**: 需要调用本地 git 命令或等价 Git 信息读取路径，并为配置解析增加验证测试
