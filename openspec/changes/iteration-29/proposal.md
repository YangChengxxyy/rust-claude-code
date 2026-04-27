## Why

当前 Memory 系统仍依赖用户手动运行 `/memory remember`，环境诊断和代码审查也缺少一等 slash command 入口。迭代 29 需要补齐这些工程化能力，让 Rust 版在真实项目协作中更接近 Claude Code 的日常工作流。

## What Changes

- 增加 Auto Memory：agent 可在用户纠正错误、表达长期偏好或项目上下文发生稳定变化时自动保存记忆
- 为自动记忆写入增加去重与更新策略，并提供 `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` 禁用开关
- 增加 `/doctor` slash command，输出 API、配置、MCP、工具和权限文件的环境诊断报告
- 增加 `/review` slash command，基于当前分支、PR 号码或 PR URL 收集 Git diff 并请求 agent 生成结构化代码审查意见
- 保持现有手动 `/memory remember`、记忆读取、权限系统和 QueryLoop 工具执行模型兼容

## Capabilities

### New Capabilities
- `auto-memory`: agent 自动识别并持久化适合长期保存的用户偏好、反馈和项目上下文
- `doctor-command`: `/doctor` 提供本地运行环境和项目配置诊断报告
- `review-command`: `/review` 收集 Git/PR 变更并生成结构化代码审查意见

### Modified Capabilities
- `memory-command-lite`: `/memory remember` 和自动记忆写入共享去重感知的保存路径
- `memory-dedup-correction`: 去重检测支持自动记忆写入场景并优先更新既有记忆
- `memory-maintenance`: 记忆维护工作流正式接入自动提取和更新路径
- `memory-contract`: prompt 构造增加自动记忆何时保存、何时避免保存、如何尊重禁用开关的行为约束
- `slash-command-extensions`: 内置 slash command 集合增加 `/doctor` 和 `/review`

## Impact

- **`crates/core/src/memory.rs`**: 增加自动记忆候选、禁用开关判断、去重写入复用入口
- **`crates/cli/src/system_prompt.rs` / `crates/cli/src/query_loop.rs`**: 注入自动记忆指引并在 agent 轮次中支持自动记忆保存请求
- **`crates/tui/src/app.rs` / `crates/tui/src/ui.rs` / `crates/tui/src/events.rs`**: 注册并分发 `/doctor`、`/review`，显示诊断和审查结果
- **Git 与外部命令调用**: `/review` 需要非交互式运行 `git diff` / `git status`，PR URL 或号码可优先通过 `gh` 获取远端变更信息，缺失时回退当前分支 diff
- **测试**: 需要覆盖自动记忆禁用、去重更新、doctor 报告、review diff 收集和 slash command 注册行为
