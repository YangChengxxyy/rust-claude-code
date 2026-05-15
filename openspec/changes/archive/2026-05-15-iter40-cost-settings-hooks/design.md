## Context

迭代 40 汇总了三个第四期规划中的小型但横跨运行时边界的缺口：成本估算、配置迁移、Hook 生命周期补全。当前项目已经有 usage 累计、settings/config 读取、hook 配置与执行、以及 slash command registry，因此本迭代应优先复用现有路径，而不是引入独立运行时框架。

## Goals / Non-Goals

**Goals:**
- 在 `core` 中提供模型定价、调用成本计算、会话成本累计和预算状态判断。
- 让 `/cost` 使用模型感知与缓存感知的成本明细，而不是单一粗略估算。
- 在配置加载路径中加入版本化迁移，支持对持久化 `config.json` 做可测试升级。
- 让 Hook 配置支持 `once`，并在 CLI 会话开始/结束时触发生命周期 hook。

**Non-Goals:**
- 不接入实时账单 API 或远端价格表。
- 不实现跨进程或跨会话的成本持久化。
- 不为所有配置来源做迁移；迁移只作用于项目自己的 persisted config file。
- 不扩展 hook 的 command 以外类型。

## Decisions

### Decision: 成本能力放在 `core` crate

`Usage` 类型和配置类型都在 `core`，成本计算是纯函数逻辑，放在 `core::cost` 可以被 CLI、TUI 和 SDK 复用。`CostTracker` 只保存内存态的本会话累计记录，不写入磁盘。

Alternative considered: 在 `cli` 中实现 `/cost` 专用计算。拒绝原因是会让 TUI/SDK 复用困难，也会把模型价格和预算语义藏在 UI 层。

### Decision: 使用静态模型名匹配表

`get_pricing(model)` 通过小写模型名包含 `opus`、`sonnet`、`haiku` 匹配价格，未知模型使用 Sonnet 价格。这样不增加网络依赖，也符合当前配置解析模型字符串的方式。

Alternative considered: 使用精确 model ID 表。拒绝原因是模型 ID 更新频繁，包含匹配更适合当前项目的轻量实现。

### Decision: 配置迁移直接操作 JSON Value

迁移 trait 接收 `serde_json::Value`，这样可以在反序列化为强类型 `Config` 之前修正字段形态和旧值。版本号存储在顶层 `_migration_version` 字段，迁移完成后再写回文件。

Alternative considered: 对 `Config` 结构做迁移。拒绝原因是旧配置可能无法反序列化成最新结构，无法覆盖复杂迁移。

### Decision: Hook once 状态保存在 HookRunner 内存中

`HookRunner` 维护执行过的一次性 hook key 集合。key 由事件、matcher 和 command 等配置组成，使同一 session 内的同一 hook 只运行一次。状态不持久化，下一次 CLI session 重新开始。

Alternative considered: 在 settings 或 session 文件中持久化 once 状态。拒绝原因是 `once` 的需求是“每会话一次”，持久化会改变语义。

### Decision: SessionStart/SessionEnd 在 CLI runtime 边界触发

CLI main/query-loop 外层拥有 session id、cwd、model、permission mode、开始时间、最终 usage/cost 和消息数量，因此生命周期 hook 从 CLI 集成层触发。HookRunner 继续只负责匹配和执行。

Alternative considered: 在 QueryLoop 内触发生命周期 hook。拒绝原因是 QueryLoop 关注单次 agentic request，无法覆盖 TUI、非交互模式和退出路径的完整生命周期。

## Risks / Trade-offs

- [Risk] 静态价格表会随真实 API 定价变化而过期 → Mitigation: 将价格集中在 `core::cost::get_pricing`，后续只需更新一个模块。
- [Risk] 退出时触发 SessionEnd 可能被异常终止跳过 → Mitigation: 覆盖正常退出和 Ctrl+C 路径，不承诺 kill -9 等不可捕获终止。
- [Risk] 配置迁移写回失败可能影响启动 → Mitigation: 迁移失败返回明确错误，不在部分写回后继续使用不确定配置。
- [Risk] once hook key 选择过粗会误跳过不同 hook → Mitigation: key 包含事件、matcher、command 和 timeout。

## Migration Plan

1. 新增 `core::cost` 和 `core::migration`，并导出公共类型。
2. 在 config loading 路径中运行默认迁移 runner。
3. 扩展 hook config/execution 类型和测试。
4. 集成 CLI session start/end hook 与 `/cost` 明细输出。
5. 运行 workspace check/test。

Rollback: 删除本迭代新增模块和调用点即可恢复旧行为；迁移写入的 `_migration_version` 字段对旧 serde 配置应被忽略。

## Open Questions

无。