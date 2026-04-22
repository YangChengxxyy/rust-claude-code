## Context

当前 Rust Claude Code 已完成 17 个迭代，具备完整的 QueryLoop、权限系统、TUI 交互、配置合并（user/project settings.json）和 slash command 能力。但缺乏任何自动化扩展点 — 用户无法在工具执行前后注入自定义逻辑。

原版 TS Claude Code 有一个完整的 hooks 系统，支持 27 种事件类型和 4 种 hook 类型（command/prompt/agent/http）。本迭代聚焦在最核心的子集：command 类型 hook + 5 个高频事件。

**当前代码集成点**：
- `QueryLoop::execute_tool_uses()` (query_loop.rs:489-601) — 工具执行前后
- `ClaudeSettings` (settings.rs) — hooks 配置加载
- `TuiBridge` (bridge.rs) — UI 事件通知
- `main.rs` — 启动时加载配置、用户输入提交

## Goals / Non-Goals

**Goals:**
- 实现 command 类型 hook，通过 shell 执行用户自定义脚本
- 支持 5 个核心事件：`PreToolUse`、`PostToolUse`、`UserPromptSubmit`、`Stop`、`Notification`
- Hook 配置格式与 TS 版本 `settings.json` 的 `hooks` 字段兼容
- Hook 通过 stdin/stdout 传递 JSON 输入/输出
- PreToolUse hook 支持 approve/block 决策
- Hook 执行有超时保护，不阻塞主流程
- 从 user + project 级 settings.json 加载并合并 hook 配置

**Non-Goals:**
- 不实现 `prompt`、`agent`、`http` 三种 hook 类型（留待后续迭代）
- 不实现全部 27 种事件类型（仅实现 5 个核心事件）
- 不实现 async/background hook 执行模式
- 不实现 `once` 自动移除和 `asyncRewake` 模式
- 不实现 hook 对工具输入的修改能力（`updatedInput`）— 留待后续
- 不实现 plugin 系统相关的环境变量（`CLAUDE_PLUGIN_ROOT` 等）
- 不实现 workspace trust 安全检查

## Decisions

### D1: Hook 配置结构 — 与 TS 版本兼容但裁剪

**选择**: 采用与 TS 版本相同的 JSON 结构，但只支持 `type: "command"` 的 hook。

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "/path/to/script.sh"
          }
        ]
      }
    ]
  }
}
```

**理由**: 保持配置格式兼容，用户可以逐步迁移 TS 版本的 hook 配置。不支持的 `type` 在加载时发出警告并跳过。

**备选方案**: 自定义更简洁的配置格式 — 否决，因为会引入不必要的迁移成本。

### D2: Hook 执行架构 — `HookRunner` 独立模块在 cli crate

**选择**: 在 `cli` crate 新增 `hooks.rs` 模块，包含 `HookRunner` struct。`HookRunner` 持有 hook 配置的引用，提供 `run_hooks(event, input)` 方法。

**理由**: 
- Hook 执行涉及 subprocess spawn（`tokio::process::Command`），属于 CLI 层逻辑
- `core` crate 只定义类型（`HookEvent`、`HookConfig`、`HookResult`），保持零外部依赖
- `HookRunner` 通过 `Arc<HookRunner>` 注入 `QueryLoop`，不改变现有签名

**备选方案**: 放在 tools crate — 否决，hooks 不是工具，是横切关注点。

### D3: Hook 输入输出通信 — JSON via stdin/stdout

**选择**: 
- 输入：通过 stdin 传入 JSON（包含 session context + event-specific 字段）
- 输出：从 stdout 读取 JSON 响应（`{ "decision": "approve"|"block", "reason": "..." }`）
- 环境变量：注入 `CLAUDE_PROJECT_DIR` 和 `HOOK_EVENT`

**理由**: 与 TS 版本一致，shell script 可以用 `jq` 等标准工具处理 JSON。

### D4: Hook 匹配 — matcher 字段做工具名前缀匹配

**选择**: `matcher` 字段支持工具名匹配（如 `"Bash"`、`"Write"`、`"Read"`）。空 matcher 或 `""` 匹配所有工具。对于 `PreToolUse` 和 `PostToolUse`，matcher 按工具名匹配；对其他事件，matcher 被忽略。

**理由**: 覆盖最常见的用例（"对所有 Bash 命令执行前检查"），简化实现。TS 版本的 permission rule 语法（`Bash(git *)`）留待后续迭代支持。

### D5: 超时策略 — 默认 10 秒，可配置

**选择**: Hook 执行默认超时 10 秒。可通过 hook 配置的 `timeout` 字段覆盖（单位秒）。超时时视为 hook 失败但不阻塞，发出警告继续执行。

**理由**: 防止用户脚本挂起阻塞整个 CLI。10 秒足够大多数检查脚本。

### D6: Hook 结果处理 — 仅 PreToolUse 支持 block 决策

**选择**: 
- `PreToolUse`: 支持 `decision: "approve"` | `"block"`。block 时拒绝工具执行，返回错误给模型。
- `PostToolUse`: 仅读取（不影响已执行的结果），支持 `additionalContext`。
- `UserPromptSubmit`、`Stop`、`Notification`: 仅触发，不处理返回值。
- Hook stdout 为空或解析失败时，默认 `approve`（不阻塞）。

**理由**: 最小可用原则。先实现最重要的 block 能力，其他事件先只支持通知。

### D7: Hook 配置合并 — 与 permissions 相同策略

**选择**: user 级和 project 级的 hooks 配置按事件合并（concatenate），不覆盖。同一事件下的 hook 列表按配置源顺序排列（project 在 user 之后）。

**理由**: 与现有 `permissions.allow/deny` 的合并策略一致。

### D8: QueryLoop 集成 — 注入 `Option<Arc<HookRunner>>`

**选择**: `QueryLoop` 新增 `hook_runner: Option<Arc<HookRunner>>` 字段。在 `execute_tool_uses()` 中：
- 权限检查后、实际执行前：调用 `hook_runner.run_pre_tool_use(tool_name, input)`
- 工具执行后：调用 `hook_runner.run_post_tool_use(tool_name, input, output)`

如果 `hook_runner` 为 `None`，跳过所有 hook（向后兼容）。

**理由**: 保持 QueryLoop 对 hooks 的可选依赖，不影响测试和无 hook 场景。

## Risks / Trade-offs

**[R1: Hook 脚本安全性]** → 信任用户配置。Hook 以当前用户权限执行 shell 命令，恶意 project settings 可能执行危险命令。
→ **缓解**: 本迭代不实现 workspace trust 检查（留待后续），但在文档中明确警告。

**[R2: Hook 执行性能]** → spawn subprocess 有开销。每次工具调用前后都 spawn 新进程。
→ **缓解**: 超时保护 + 只在有匹配 hook 时才 spawn。大多数用户只配置少量 hook。

**[R3: Hook stdout 不可靠]** → 用户脚本可能输出非 JSON 内容或混入 stderr。
→ **缓解**: stdout 解析失败时默认 approve（不阻塞），stderr 被捕获但不解析。

**[R4: 与 TS 版本兼容性有限]** → 只支持 command 类型，忽略 prompt/agent/http 类型。
→ **缓解**: 不支持的类型在加载时 warn 并跳过，不报错。用户可逐步迁移。
