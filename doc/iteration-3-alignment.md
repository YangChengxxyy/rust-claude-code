# 迭代 3 对齐记录

## 1. 目的

本文件用于记录当前 Rust 实现与 Claude Code 参考源码之间的差距，并给出迭代 3 期间的设计取舍。它不是功能规划文档，而是后续实现的约束文档。

适用范围：

- `crates/core`
- `crates/api`
- 后续会依赖这些公共边界的迭代 4、7、8

---

## 2. 对齐结论总览

| 主题 | 状态 | 说明 |
|------|------|------|
| 消息模型 | 部分对齐 | 已采用 role + content blocks，但 `system` 的承载方式仍需明确 |
| ToolResult 模型 | 部分对齐 | 已有独立类型，但与 `ContentBlock::ToolResult` 仍未完全收口 |
| 权限模型 | 部分对齐 | 基础模式与规则已落地，管理器与持久化尚未实现 |
| AppState | 部分对齐 | 已有最小状态容器，但会话配置边界仍待校准 |
| API 客户端 | 部分对齐 | 已支持非流式消息创建，但扩展边界仍偏直接 |

---

## 3. 分项差距记录

### 3.1 消息模型

**已对齐**

- 当前 Rust 版本已采用 `Message { role, content }` 结构。
- `ContentBlock` 已支持 `Text`、`ToolUse`、`ToolResult`、`Thinking`。
- `Usage` 已支持输入输出 token 统计，并兼容 cache token 计数。

**部分对齐**

- 参考设计中，system 内容在整体协议中拥有独立语义；Rust 当前实现主要通过 API 层 `CreateMessageRequest.system` 表达，而非 `Role::System`。
- 该设计当前可以工作，但需要在后续 Query Loop 和会话管理中确认是否继续沿用。

**未对齐**

- 尚未最终确定 system 内容是否需要统一进入 `Message` 历史，还是继续只留在请求层。

**本轮取舍**

- 先不引入 `Role::System`。
- 优先保持 API 请求层 `system` 字段与 Anthropic 协议直接对齐。
- 等 Query Loop 和 System Prompt 迭代时再决定是否需要额外的会话层抽象。

### 3.2 ToolResult 模型

**已对齐**

- `crates/core/src/tool_types.rs` 中已存在独立 `ToolResult`。
- 工具结果支持 `tool_use_id`、`content`、`is_error` 与基础 metadata。

**部分对齐**

- `ToolResult::to_content_block()` 最终仍会转换为 `ContentBlock::ToolResult`。
- `ContentBlock::ToolResult` 当前只有 `tool_use_id`、`content: String`、`is_error`，无法完整表达 `ToolResultMetadata`。

**未对齐**

- 工具结果内容是否需要支持结构化内容块，目前仍未收口。
- 工具执行层与消息层之间的边界尚不够清晰，后续可能影响 Query Loop 的拼接逻辑。

**本轮取舍**

- 先不为了未来需求提前引入复杂的结构化 ToolResult 内容模型。
- 但在 `core` 里要把“工具执行结果”和“消息块表达”视为两个独立层次，避免未来继续把 metadata 悄悄丢失。

### 3.3 权限模型

**已对齐**

- `PermissionMode` 已覆盖 `default`、`acceptEdits`、`bypassPermissions`、`plan`、`dontAsk`。
- `PermissionRule` 已支持 `tool_name` 与可选 `pattern`。
- `PermissionCheck` 已能表达允许、拒绝、需要确认。
- 已通过 `check_tool_with_command()` 支持带命令内容的规则匹配。

**部分对齐**

- 当前权限类型更偏基础判定逻辑，尚未承接规则持久化、交互确认上下文与更高层管理职责。

**未对齐**

- `PermissionManager` 尚未实现。
- 规则持久化尚未实现。
- Query Loop 尚未集成权限检查流转。

**本轮取舍**

- 保持当前权限类型尽量简单，不在迭代 3 提前实现完整权限管理器。
- 迭代 3 只收敛边界定义，不扩大权限功能范围。

### 3.4 AppState

**已对齐**

- 当前 `AppState` 已包含：
  - `messages`
  - `todos`
  - `permission_mode`
  - `model`
  - `max_tokens`
  - `cwd`
  - `total_usage`

**部分对齐**

- 这些字段足以支撑当前已完成能力，但是否应继续平铺会话配置，还是收敛成专门的会话配置对象，尚未决定。

**未对齐**

- 尚未定义 Query Loop、System Prompt、会话管理所需的最小稳定字段集合。

**本轮取舍**

- 不在迭代 3 把 `AppState` 扩展成大而全的运行时容器。
- 优先识别后续迭代真正需要的最小字段，避免为未实现能力提前加状态。

### 3.5 API 客户端

**已对齐**

- 已实现非流式 `AnthropicClient`。
- 已支持 `base_url`、`anthropic-version`、`x-api-key`。
- 已具备基础错误映射与示例程序。

**部分对齐**

- 当前客户端实现能工作，但请求构造、header 构造和 endpoint 组织仍偏直接。
- 当前类型层已经支持 `stream`，但客户端尚未抽出流式与非流式共享边界。

**未对齐**

- `metadata` 尚未进入请求模型。
- provider 扩展点尚未清晰定义。
- retry 策略与流式复用边界尚未建立。

**本轮取舍**

- 先不抽象通用 provider trait。
- 先把 Anthropic 客户端内部的请求边界整理干净，再为流式和重试预留复用点。

---

## 4. 本轮实施约束

迭代 3 的代码修改应遵守以下约束：

1. 不为了“看起来更像官方”而提前复制整套 TypeScript 架构。
2. 不引入与当前迭代目标无关的 runtime 状态字段。
3. 不引入只为兼容初版设计而存在的临时包装层。
4. 优先收敛公共类型与 API 边界，而不是新增用户可见功能。
5. 保持现有测试通过，并让后续迭代可以直接复用这些边界。

---

## 5. 本轮不处理项

以下内容明确不属于迭代 3 的实现范围：

- SSE 流式解析与事件累积
- Query Loop 正式实现
- 完整权限管理器与规则持久化
- 工具具体实现（Bash / FileRead / FileEdit / FileWrite / TodoWrite）
- TUI 或 CLI 交互能力完善
- 会话持久化与 slash commands

这些内容将在后续对应迭代中处理，但必须遵守本文件记录的公共边界约束。
