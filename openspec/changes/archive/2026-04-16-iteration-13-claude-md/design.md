## Context

当前 Rust Claude Code 的 system prompt 由 `cli/system_prompt.rs` 中的 `build_system_prompt()` 函数构建，包含核心行为指南、工具描述和环境信息三个部分。支持 `--append-system-prompt` 参数附加自定义内容，但不具备自动发现和注入项目级指令（CLAUDE.md）的能力。

原版 Claude Code 通过 CLAUDE.md 机制让用户以 Markdown 文件的形式提供项目级上下文（代码规范、构建命令、架构约定等），助手会自动发现这些文件并注入 system prompt。这是提升代码生成质量的核心机制之一。

当前实现依赖：迭代11已完成 system prompt 构建和会话持久化功能。

## Goals / Non-Goals

**Goals:**
- 实现从工作目录向上遍历发现所有 `CLAUDE.md` 文件
- 支持全局用户级指令文件 `~/.claude/CLAUDE.md`
- 将指令内容按确定性顺序合并并注入 system prompt
- 提供截断保护避免指令过长撑爆上下文
- 保持与原版 Claude Code CLAUDE.md 格式的兼容性

**Non-Goals:**
- 不支持 `.claudeignore` 排除规则（留给后续迭代）
- 不支持动态热重载（启动时加载一次即可）
- 不支持 CLAUDE.md 的编辑 UI 或 slash command 管理
- 不支持 CLAUDE.local.md 等变体文件（可后续扩展）
- 不实现信任边界和权限提示（原版的 "Trust this file?" 机制留给后续）

## Decisions

### 1. 文件发现策略：向上遍历 + 全局文件

**选择**: 从 CWD 开始，沿父目录链向上直到文件系统根目录（或 git repo 根目录），收集所有 `CLAUDE.md` 文件；同时检查 `~/.claude/CLAUDE.md` 作为全局指令。

**替代方案**:
- 仅搜索 CWD 和 git root —— 太简化，不支持 monorepo 中间层
- 搜索所有子目录 —— 不必要且性能差

**理由**: 与原版 Claude Code 行为一致。向上遍历可以支持 monorepo 中子项目有自己的 CLAUDE.md，同时继承父目录/根目录的通用指令。以 git root 为上限可以避免不必要地扫描到用户 home 目录以上的路径。

### 2. 合并顺序：全局 → 祖先到叶

**选择**: 合并顺序为 `全局(~/.claude/CLAUDE.md)` → `git root CLAUDE.md` → `中间目录 CLAUDE.md` → `CWD CLAUDE.md`。

**理由**: 更具体的指令（越靠近工作目录）排在后面，在 system prompt 中位置更靠后，模型会给予更高权重。这与原版行为一致，也符合"越具体越优先"的直觉。

### 3. 新模块放置：`core` crate 中新增 `claude_md` 模块

**选择**: 将 CLAUDE.md 发现和加载逻辑放在 `core` crate。

**替代方案**:
- 放在 `cli` crate —— 可行，但如果 `tui` 也需要用到就得重复
- 新建独立 crate —— 过度抽象

**理由**: `core` 是零外部服务依赖的共享类型库，文件发现是纯 IO 操作（只用 `std::fs` 和 `std::path`），放在 core 中任何其他 crate 都能使用。

### 4. System prompt 注入位置：紧跟在环境信息之后

**选择**: 在 `build_system_prompt()` 中，将 CLAUDE.md 内容作为 `# claudeMd` section 插入在环境信息之后、自定义 append 之前。

**理由**: 与原版 Claude Code 的 system prompt 结构对齐。项目指令属于上下文信息，逻辑上应在工具描述和环境信息之后，但在用户自定义附加内容之前。

### 5. 截断策略：硬上限 + 文件尾部优先

**选择**: 设定 CLAUDE.md 合并后的总字符数上限（默认 30000 字符）。如果超限，从全局文件开始截断（保留更具体的项目级指令）。

**理由**: 避免超大 CLAUDE.md 文件消耗过多 system prompt 空间。30000 字符约 7500 tokens，在 system prompt 总预算中是合理的上限。

## Risks / Trade-offs

- **[性能] 文件系统遍历开销** → 在深层嵌套目录中向上遍历可能涉及多次 `fs::read_to_string` 调用。Mitigation: 仅在启动时执行一次，且 `CLAUDE.md` 文件通常很小。
- **[正确性] 符号链接循环** → 向上遍历时可能遇到符号链接导致的路径循环。Mitigation: 使用 `canonicalize()` 解析真实路径，检测已访问目录。
- **[兼容性] 编码问题** → CLAUDE.md 文件可能不是 UTF-8 编码。Mitigation: 使用 `fs::read_to_string()` 会自动 fallback，非 UTF-8 文件跳过并打 warning。
- **[安全性] 不可信目录中的 CLAUDE.md** → 恶意 CLAUDE.md 可能注入有害指令。Mitigation: 当前不实现信任边界（non-goal），后续迭代可增加用户确认机制。
