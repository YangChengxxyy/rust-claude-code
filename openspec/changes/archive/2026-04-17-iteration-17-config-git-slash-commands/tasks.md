## 1. 配置发现与合并

- [x] 1.1 在 `crates/core` 中扩展 settings/config 类型，支持项目级 `.claude/settings.json`、`permissions` 字段与配置来源元数据建模
- [x] 1.2 实现从当前工作目录向上发现项目级 `.claude/settings.json` 的逻辑，并与现有 git root / CLAUDE.md 路径归一化边界复用
- [x] 1.3 在 `crates/cli` 中收敛配置优先级链，确保 CLI > env > project settings > user settings > defaults，并补充表驱动测试
- [x] 1.4 实现 settings 结构校验与错误处理，确保 malformed `permissions` 等字段不会静默生效

## 2. Git 上下文集成

- [x] 2.1 定义 Git context snapshot 类型与采集入口，覆盖 repo root、branch、clean 状态、recent commits 与非 Git 目录降级路径
- [x] 2.2 将 Git context 注入 `crates/cli` 的 system prompt 生成流程，并补充有/无 Git 仓库场景测试
- [x] 2.3 在 `crates/tui` 状态栏接入 Git branch 展示与刷新逻辑，确保分支切换或工作树变化后可更新显示

## 3. Slash command 注册与新命令

- [x] 3.1 将现有 slash commands 收敛为统一注册表，保证 `/help` 输出与命令分发使用同一份定义
- [x] 3.2 实现 `/config`，显示关键运行时配置值及其来源，包括 model 与 permission 配置
- [x] 3.3 实现 `/cost`，显示当前会话累计 usage 与基于活动 model 的成本估算或占位输出
- [x] 3.4 实现 `/diff`，在 Git 仓库中展示当前工作区 diff，在 clean 或非 Git 场景下给出稳定提示
- [x] 3.5 增强 `/clear`，支持显式 preserve-context 模式且保持默认行为不变

## 4. 集成验证

- [x] 4.1 运行相关单元测试，至少覆盖 `cargo test -p rust-claude-core -p rust-claude-cli -p rust-claude-tui`
- [x] 4.2 手动运行 `cargo run -p rust-claude-cli`，验证项目级 settings、生效来源展示、branch 状态栏、`/diff`、`/cost`、`/config`、增强版 `/clear`
