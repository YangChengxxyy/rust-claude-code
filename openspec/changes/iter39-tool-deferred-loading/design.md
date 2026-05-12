## Context

当前系统每次发送 API 请求时，都会将 `ToolRegistry` 中所有注册工具的完整 JSON Schema（含 `name`、`description`、`input_schema`）序列化到 `CreateMessageRequest.tools` 中。随着 MCP 工具、WebSearch、LSP 等扩展工具的接入，工具列表可能达到 20+ 个，占用数千到上万 token 的 prompt 空间。

Anthropic API 支持仅发送工具名称（不带 schema），模型可以在需要时通过其他方式发现工具。原版 Claude Code 使用 `shouldDefer` 标记低频工具，只发送名称，模型通过 `ToolSearchTool` 按需加载完整 schema。

本项目当前架构中：
- `Tool` trait 位于 `tools` crate，已有 `is_read_only`、`is_concurrency_safe`、`interrupt_behavior`
- `ToolRegistry` 管理所有已注册工具，通过 `list()` 遍历生成 `ApiTool`
- `ApiTool` 位于 `api` crate，结构为 `{ name, description, input_schema }`
- `QueryLoop` 在每次请求前调用 `tools.list()` 构建完整工具列表

## Goals / Non-Goals

**Goals:**
- 为 `Tool` trait 增加延迟加载能力，允许特定工具仅发送名称（不含 schema）
- `ToolRegistry` 区分延迟/非延迟工具，支持动态搜索和 schema 估算
- `ApiTool` 支持 `deferred` 标记，序列化时省略 `description` 和 `input_schema`
- `QueryLoop` 维护已发现工具集合，已发现的延迟工具自动升级为完整发送
- 自动阈值控制：仅当延迟有意义时才启用（延迟工具 schema token 估算 > 上下文窗口 10%）
- 提供 `ToolSearchTool` 供模型按需搜索和发现延迟工具

**Non-Goals:**
- 改变现有工具的语义或执行行为
- 实现客户端侧模型能力检测来决定哪些工具延迟
- 支持工具 schema 的部分发送（如只发部分字段）
- 修改 Anthropic API 的流式协议

## Decisions

### Decision: `ApiTool` 通过可选字段表示延迟，而非新类型

**Rationale**: 最小化 API 层改动。`ApiTool` 增加 `deferred: Option<bool>`，当 `deferred == Some(true)` 时，序列化结果仅包含 `name` 和 `deferred: true`。使用 `Option<bool>` 配合 `skip_serializing_if` 保持向后兼容，现有非延迟工具序列化不变。

**Alternative considered**: 创建 `ApiToolRef` 枚举（`Full` / `Deferred`）。拒绝原因：需要修改 `CreateMessageRequest` 的泛型或大量构造函数调用，侵入性太大。

### Decision: 在 `QueryLoop` 中维护 `discovered_tools`，而非在 `ToolRegistry` 中

**Rationale**: 工具发现是会话级别的状态（不同会话可能发现不同工具），而 `ToolRegistry` 是全局/静态的。`QueryLoop` 持有 `discovered_tools: HashSet<String>`，每轮请求时从 registry 读取最新状态并叠加已发现工具。

**Alternative considered**: `ToolRegistry` 维护全局发现状态。拒绝原因：子 Agent、测试和并发会话会互相污染。

### Decision: 阈值基于估算 token 数，而非实际计算

**Rationale**: 精确 token 计算需要 tiktoken 或类似库，增加依赖且对中文支持复杂。使用启发式估算（schema JSON 字符串长度 / 4）足够判断是否需要延迟，且实现简单。

**Threshold logic**: 
```
if deferred_estimated_tokens > context_window * 0.10 {
    enable_deferred_loading = true;
}
```
上下文窗口大小按当前模型固定映射（如 sonnet 200K，haiku 200K）。

### Decision: `ToolSearchTool` 自身不可延迟

**Rationale**: 如果搜索工具本身被延迟，模型无法发现它，形成死锁。

### Decision: 延迟工具的发现状态仅在内存中保持，不持久化到会话文件

**Rationale**: 发现状态可以从消息历史中重建（已使用的工具会在历史中存在），且持久化增加复杂度。重启后模型重新使用工具会自动重新发现。

## Risks / Trade-offs

- **[Risk] 模型不知道延迟工具的存在，导致可用性下降** → Mitigation: 系统提示中说明存在延迟工具，并告知模型可通过 `ToolSearchTool` 搜索。ToolSearchTool 自身始终可用。
- **[Risk] ToolSearchTool 搜索结果不准确** → Mitigation: 实现 CamelCase 和 `mcp__` 名称拆分，支持精确选择语法 `select:ToolName`，名称匹配权重 2x。
- **[Risk] 所有工具被过滤后只剩延迟工具，模型无法使用** → Mitigation: 核心高频工具（Bash、FileRead、FileEdit、FileWrite、Grep、Glob）始终不延迟。
- **[Risk] 向后兼容：旧代码直接构造 `ApiTool::new()` 不设置 deferred** → Mitigation: `deferred` 默认为 `None`，行为与之前完全一致。

## Migration Plan

本变更为纯新增功能，无迁移或回滚需求。旧配置和旧会话无需处理。

部署步骤：
1. 实现 `should_defer`、`ApiTool.deferred`、`ToolRegistry` 新方法
2. 标记目标工具为延迟
3. 实现 `ToolSearchTool` 并注册到 registry
4. 修改 `QueryLoop` 动态组装工具列表
5. 添加单元测试覆盖搜索评分和阈值逻辑

## Open Questions

- (已解决) 阈值是否应该可配置？→ 先使用固定 10%，后续根据反馈调整。
