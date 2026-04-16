## Why

Rust Claude Code 目前没有项目指令系统。用户无法通过 `CLAUDE.md` 文件向助手提供项目级别的规范、约定和上下文信息。这是与原版 Claude Code 对齐的关键缺失功能——原版通过发现并注入 `CLAUDE.md` 内容到 system prompt，使助手能感知项目规范，大幅提升代码生成质量和一致性。

## What Changes

- 新增 CLAUDE.md 文件发现机制：从当前工作目录向上遍历父目录，收集所有 `CLAUDE.md` 文件
- 支持用户级全局指令文件（`~/.claude/CLAUDE.md`）
- 将发现的项目指令内容注入 system prompt 构建流程，以 `# claudeMd` section 的形式插入
- 定义多份指令文件的合并顺序：全局 → 祖先目录（从根到叶）→ 当前目录
- 支持 `.claudeignore` 或特定规则排除某些目录的 CLAUDE.md
- 实现截断保护，避免过长的指令内容撑爆 system prompt

## Capabilities

### New Capabilities
- `claude-md-discovery`: CLAUDE.md 文件发现与加载——从工作目录向上遍历，收集全局和项目级指令文件
- `claude-md-injection`: 将已发现的 CLAUDE.md 内容合并并注入 system prompt

### Modified Capabilities

（无现有 spec 级别的需求变更）

## Impact

- **`core` crate**: 新增 `claude_md` 模块，提供文件发现和内容加载逻辑
- **`cli` crate**: 修改 `system_prompt.rs` 中的 `build_system_prompt()` 函数，增加 CLAUDE.md 注入步骤
- **`cli/main.rs`**: 在启动时调用 CLAUDE.md 发现逻辑，将结果传入 system prompt 构建
- **无新外部依赖**: 仅使用标准库的 `fs` 和 `path` 功能
- **不影响现有 API/工具行为**: 纯 system prompt 增强，对消息格式和工具系统透明
