## Context

迭代 29 横跨 memory、system prompt、QueryLoop、TUI slash commands 和 Git/环境检查流程。现有代码已经具备 typed memory store、`/memory remember|forget`、去重更新、CLAUDE.md 与 memory prompt 注入、MCP 配置读取、Git context、TUI command registry 和后台 `UserCommand` 分发，因此本次应复用这些基础能力，而不是引入新的长期状态系统。

## Goals / Non-Goals

**Goals:**
- 让 agent 在明确的系统提示和安全边界下自动保存长期有效的记忆
- 让自动记忆与手动 `/memory remember` 共享同一套去重、更新和索引重建逻辑
- 在 TUI 中提供 `/doctor`，用单一报告展示 API、配置、MCP、工具和权限文件健康状态
- 在 TUI 中提供 `/review`，从 PR 参数或当前分支收集 diff，并通过现有 QueryLoop 生成结构化 review
- 保持所有新增行为可测试、可禁用、可回退

**Non-Goals:**
- 不实现完整交互式 memory 管理 UI、文件选择器或编辑器集成
- 不引入向量数据库、embedding 或跨项目语义检索
- 不实现完整 GitHub API 客户端；`gh` 可用时复用，否则回退本地 Git diff
- 不让 `/review` 直接发布 PR 评论或修改代码
- 不改变现有 permission mode、tool registry 或 MCP 传输模型

## Decisions

1. **自动记忆使用系统提示加受控写入入口，而不是后台启发式扫描。**
   - Rationale: 用户纠正、偏好表达和稳定项目上下文通常只在对话语义中可见，由模型识别更自然；Rust 侧负责校验、禁用开关和持久化。
   - Alternative considered: 在 Rust 侧对每条用户消息做关键词提取。该方案简单但误报高，且无法理解“不要记住”之类语义边界。

2. **自动记忆复用 `MemoryWriteRequest`、`find_duplicate_memory` 和 correction path。**
   - Rationale: 手动记忆和自动记忆都应遵守 topic-file-first、index rebuild 和去重更新规则，避免产生两套不一致的文件格式。
   - Alternative considered: 为自动记忆新增独立目录。该方案降低冲突但会破坏 `/memory` inspection 和现有 recall selection。

3. **禁用开关在 Rust 侧强制执行。**
   - Rationale: `CLAUDE_CODE_DISABLE_AUTO_MEMORY=1` 必须可靠禁止自动写入，即使 prompt 仍被旧上下文引用也不能保存。
   - Alternative considered: 只在 prompt 中告诉模型不要保存。该方案无法作为安全边界。

4. **`/doctor` 生成本地诊断报告，不通过模型总结。**
   - Rationale: 环境健康检查应快速、确定、可测试，并且不依赖 API 可用性本身。
   - Alternative considered: 将诊断输入交给 agent 解释。该方案在 API 故障时不可用，并且会使报告格式不稳定。

5. **`/review` 把 diff 收集与 review 生成分离。**
   - Rationale: Rust 侧负责非交互式收集当前分支、PR URL 或 PR 号码对应的变更；审查意见由现有 QueryLoop 生成，复用模型、权限、streaming 和 TUI 输出路径。
   - Alternative considered: 在 TUI 里直接实现静态规则审查。该方案难以覆盖语义问题，也偏离 agentic review 目标。

6. **PR 输入优先使用 `gh`，缺失时回退本地 Git。**
   - Rationale: `gh` 已是工程常见工具，能支持 PR URL/号码；本地 diff 回退保证没有 GitHub CLI 时 `/review` 仍可用于当前分支。
   - Alternative considered: 直接调用 GitHub REST API。该方案需要额外认证和 API 类型，不符合本迭代范围。

## Risks / Trade-offs

- [Risk] 自动记忆可能保存过多或保存不该持久化的信息 → Mitigation: prompt 明确保存条件和禁止项，Rust 侧支持全局禁用，并复用去重更新减少噪音
- [Risk] 自动记忆 tool/write path 可能与用户显式 `/memory forget` 冲突 → Mitigation: correction path 只在当前扫描结果中更新，forget 后不保留隐藏 tombstone；用户后续纠正可重新写入
- [Risk] `/doctor` API 连通性检查可能消耗请求或受网络影响 → Mitigation: 报告将 API 检查标为独立项，失败不阻塞其他检查，优先使用轻量请求或配置可解析性检查
- [Risk] `/review` diff 可能过大导致上下文膨胀 → Mitigation: 先收集 summary，再按既有限制截断 diff，并在 prompt 中声明截断状态
- [Risk] `gh` 不可用或 PR 不属于当前 repo → Mitigation: 明确报告回退原因，并使用当前分支 diff 或返回可操作错误
- [Risk] TUI 命令增多导致 registry 与 dispatch 再次漂移 → Mitigation: 新命令必须注册在统一 command list，并补充 help/validation/dispatch 测试

## Migration Plan

1. 增加 core memory API：自动记忆候选类型、禁用开关检查、dedup-aware save helper。
2. 更新 system prompt memory contract，描述自动保存条件、禁止项和 disabled behavior。
3. 在 QueryLoop 或 TUI worker 中接入自动记忆保存请求，确保禁用时 no-op。
4. 增加 `/doctor` 的 `UserCommand`、检查器和 TUI 输出。
5. 增加 `/review` 的 `UserCommand`、diff 收集器和 review prompt dispatch。
6. 补充单元测试和必要的 command dispatch 测试。

Rollback strategy: 新增行为均通过新增命令和 auto-memory 禁用开关隔离；若自动记忆出现问题，可先默认禁用或仅保留手动 `/memory remember` 路径，`/doctor` 与 `/review` 可独立回退。

## Open Questions

- `/review` 的默认 base 分支应从 `git merge-base origin/main HEAD` 推断，还是优先读取配置中的默认分支？首版建议按 Git 远端 HEAD/main/master 逐级回退。
- API 连通性检查是否应发送真实模型请求？首版建议避免昂贵请求，优先检查配置和客户端构建，必要时提供轻量可选检查。
