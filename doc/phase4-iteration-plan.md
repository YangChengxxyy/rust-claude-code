# 第四期迭代计划 — 韧性、安全与功能补齐

> 生成时间: 2026-05-03
>
> 基于 `doc/2026-05-03_review.md` 的进度审查，制定从迭代 35 起的第四期规划。
>
> 第四期核心目标：**补齐韧性和安全基础设施，提升性能关键路径，清扫遗留项。**

---

## 0. 当前状态概览

第一期（迭代 1-11）：基础骨架，从零到可运行。
第二期（迭代 12-22）：核心工具补齐、MCP、Agent、Hooks、Memory，达到"能日常使用"。
第三期（迭代 23-34）：体验打磨、能力补齐、工程增强、生态扩展，大部分完成。

**第四期起点**：迭代 1-32 基本完成（90%+），功能覆盖原版约 55%。

对比原版 Claude Code 后发现以下三类差距：

| 差距类型 | 典型表现 | 影响 |
|----------|----------|------|
| **韧性差距** | 无响应式压缩、无模型降级、JSON 快照非增量写入 | 长对话崩溃、高负载不可用、大会话性能差 |
| **安全差距** | 无信任对话框、无沙箱隔离、无文件状态过期检测 | 恶意 settings.json 可执行任意命令 |
| **性能差距** | 工具等待完整响应后执行、所有工具 schema 占用 prompt、无文件缓存 | 多工具延迟高、prompt 空间浪费 |

此外还有 Phase 2-3 中的零散遗留项（TUI Task 面板、Hook 补全等）和计划外缺失（OAuth、@include、成本追踪等）。

---

## 1. 第四期分层策略

按"系统健壮性 × 用户影响"分为 3 个阶段：

```
阶段 A（迭代 35-37）: 韧性与安全     — 让系统不崩溃、不被利用
阶段 B（迭代 38-40）: 性能与补齐     — 让关键路径更快、功能更完整
阶段 C（迭代 41-43）: 生态与清扫     — 补齐认证生态、清扫所有遗留
```

---

## 2. 阶段 A：韧性与安全（迭代 35-37）

> 目标：保证长对话不崩溃、高负载有降级、会话不丢失、首次运行有安全确认。

### 迭代 35：响应式压缩 + 模型降级

**状态**: 规划中

**问题**: 当前仅有主动压缩（请求前按阈值检查），但 token 估算基于 chars/4 启发式，可能不准确。API 返回 `prompt-too-long` 或 `invalid_request_error` 时，系统直接向用户报错并停止，无恢复能力。此外，当模型过载返回连续 529 时，系统也只能等待重试，没有降级到备用模型的能力。

**目标**: 实现两层错误恢复——响应式压缩处理 prompt 超限，模型降级处理服务过载。

**产出**:

- `sdk` crate — `agent_loop.rs`:
  - 捕获 API `prompt-too-long` / `invalid_request_error` (HTTP 400, error.type = "invalid_request_error"):
    - 第一次：触发响应式压缩（调用 CompactionService），压缩后重试当前轮次
    - 第二次：尝试微压缩（清理旧工具结果为 `[Content cleared to reduce context size]`）后重试
    - 第三次：向用户报错，建议手动 `/compact`
  - 捕获连续 529 (overloaded) 错误:
    - 追踪连续 529 计数（`consecutive_overload_count`）
    - 超过 `MAX_OVERLOAD_RETRIES`（默认 3）后，若配置了 `fallback_model`，自动切换
    - 切换前：通过 `OutputSink` 通知用户 "Switched to {model} due to high demand"
    - 切换后：使用 fallback model 重试当前轮次
    - 成功后重置计数器
  - 新增 `RetryState` 结构追踪各类重试状态

- `sdk` crate — `compaction.rs`:
  - 新增 `micro_compact()` 方法：遍历消息历史，将旧的 `ToolResult` 内容替换为摘要文本
  - 微压缩策略：保留最近 N 轮（默认 3）的工具结果完整内容
  - 清理对象：Bash 输出、FileRead 内容、Grep/Glob 结果、WebSearch/WebFetch 结果

- `core` crate — `config.rs`:
  - `Config` 新增 `fallback_model: Option<String>` 字段
  - 支持通过 `settings.json` 的 `fallbackModel` 字段配置
  - 支持通过 `RUST_CLAUDE_FALLBACK_MODEL` 环境变量配置

- `api` crate — `error.rs`:
  - `ApiError` 新增 `PromptTooLong` 变体，携带 error message
  - `ApiError` 新增 `Overloaded` 变体，与 `RateLimited` 区分
  - `from_status_and_body()` 解析 HTTP 400 + "invalid_request_error" 和 HTTP 529

**验收标准**:

- 人为发送超长上下文（>200K tokens）时，系统自动压缩并恢复，不崩溃
- 微压缩后旧工具结果被清理，新工具结果完整保留
- 模拟连续 529 错误时，系统切换到 fallback model 并通知用户
- 切换后成功响应时，计数器重置
- 单元测试覆盖：MockClient 模拟 prompt-too-long 和 529 场景

**依赖**: 无

---

### 迭代 36：信任对话框 + 文件状态缓存

**状态**: 规划中

**问题**: 当前在不受信任的目录中运行时，`.claude/settings.json` 中的 `apiKeyHelper` 会被无条件执行，可能被恶意项目利用执行任意命令。此外，FileEdit/FileWrite 不检查目标文件是否在上次读取后被外部修改，可能导致覆盖用户的并发编辑。

**目标**: 首次在新目录运行时要求用户确认信任；引入文件状态缓存防止覆盖过期内容。

**产出**:

- `core` crate — `trust.rs`（新文件）:
  - `TrustManager` 结构：
    - `check_trust(project_dir: &Path) -> TrustStatus` — 检查目录是否已信任
    - `accept_trust(project_dir: &Path)` — 记录信任状态
    - 信任状态持久化到 `~/.config/rust-claude-code/trust.json`
    - 信任级联：父目录已信任时，子目录自动继承
    - Home 目录特殊处理：信任仅在内存中保持，不持久化
  - `TrustStatus` 枚举：`Trusted`, `Untrusted`, `InheritedFromParent`

- `cli` crate — `main.rs`:
  - 启动流程中，在 `apiKeyHelper` 和 `settings.json env` 加载之前执行信任检查
  - 未信任时：弹出信任对话框（TUI 模式）或拒绝执行（--print 模式，提示使用 --trust）
  - 新增 `--trust` CLI 参数：跳过信任对话框（用于 CI/自动化）

- `tui` crate:
  - 信任对话框组件：显示项目路径、安全提示、确认/取消选项
  - 首次运行体验：信任确认后继续正常流程

- `core` crate — `file_state_cache.rs`（新文件）:
  - `FileStateCache` 结构：
    - LRU 缓存（`lru` crate），最多 100 条目，最大 25MB
    - `record_read(path, content, offset, limit, is_partial_view)` — 记录文件读取
    - `get_read_state(path) -> Option<FileState>` — 查询文件读取状态
    - `is_stale(path) -> bool` — 检查文件是否在读取后被外部修改（比较 mtime）
  - `FileState` 结构：`content`, `timestamp`, `offset`, `limit`, `is_partial_view`
  - `is_partial_view` 语义：内容由系统自动注入（如 CLAUDE.md），与磁盘实际内容不同

- `tools` crate:
  - `FileReadTool`: 读取文件后调用 `cache.record_read()`
  - `FileEditTool`: 执行前检查 `cache.is_stale(path)`，过期时返回错误提示先重新读取
  - `FileWriteTool`: 同上，执行前检查 `cache.is_stale(path)`
  - 检查 `is_partial_view`：若为 true，要求先执行完整 FileRead

**验收标准**:

- 首次在新项目目录运行时，显示信任对话框
- 拒绝信任后，apiKeyHelper 不执行，settings.json env 不加载
- 接受信任后，后续运行不再询问
- 父目录已信任时，子目录自动跳过
- 文件在读取后被外部修改时，FileEdit 返回 "file has been modified since last read" 错误
- `is_partial_view` 的文件（如 CLAUDE.md 自动注入）无法直接编辑
- `--trust` 参数可跳过对话框

**依赖**: 新增 `lru` crate 依赖

---

### 迭代 37：JSONL 会话格式 + 崩溃恢复

**状态**: 规划中

**问题**: 当前会话以 JSON 完整快照存储，每次保存需序列化整个消息历史。随着对话增长，保存开销线性增加。此外如果进程崩溃或被 kill，最后一轮交互完全丢失。

**目标**: 迁移到 JSONL 追加日志格式，实现增量写入和崩溃恢复。

**产出**:

- `cli` crate — `session.rs` 重构:
  - `SessionWriter` 结构：
    - 管理 JSONL 文件句柄，持有 `BufWriter`
    - `append_message(msg: &Message)` — 追加单条消息
    - `append_event(event: SessionEvent)` — 追加会话事件（compact boundary, usage update, permission change）
    - `flush()` — 刷新缓冲区（每轮结束时自动调用）
    - 带 100ms 延迟写入（debounce），避免高频小写入
  - `SessionReader` 结构：
    - `load_from_jsonl(path) -> Result<SessionFile>` — 逐行解析 JSONL 重建完整会话
    - `load_from_json(path) -> Result<SessionFile>` — 向后兼容旧 JSON 格式
    - `load(path) -> Result<SessionFile>` — 自动检测格式（首字节 `{` vs `[`/其他）
  - `SessionEvent` 枚举：
    - `Header { id, model, cwd, created_at }` — 会话头（首行）
    - `UserMessage(Message)` / `AssistantMessage(Message)` — 消息
    - `CompactBoundary { summary }` — 压缩边界标记
    - `UsageUpdate(Usage)` — 累计 usage 更新
    - `PermissionChange { mode, rules }` — 权限变更
    - `SessionEnd { updated_at }` — 正常结束标记

  - 崩溃恢复逻辑：
    - 启动时扫描会话文件，检测缺少 `SessionEnd` 事件的会话
    - 对崩溃会话：显示 "Found interrupted session, would you like to resume?" 提示
    - `--continue` 时自动恢复最近崩溃会话

  - 文件命名和目录不变：`~/.config/rust-claude-code/sessions/{id}.jsonl`（新）/ `{id}.json`（旧）

- `sdk` crate — `agent_loop.rs`:
  - 每轮结束后调用 `session_writer.append_message()` + `flush()`
  - 压缩后写入 `CompactBoundary` 事件
  - 权限变更后写入 `PermissionChange` 事件

- `tui` crate — `app.rs`:
  - 崩溃恢复提示对话框

**验收标准**:

- 新会话以 `.jsonl` 格式存储
- 旧 `.json` 会话仍可读取和恢复
- 杀死进程后重启，崩溃会话可恢复到最后一轮写入点
- 1000 轮对话的会话，保存延迟 <10ms（仅追加，不重写）
- `list_recent_sessions()` 同时列出 `.json` 和 `.jsonl` 会话
- 压缩后的恢复正确识别 `CompactBoundary`

**依赖**: 无新外部依赖

---

## 3. 阶段 B：性能与补齐（迭代 38-40）

> 目标：优化关键路径性能，补齐功能型缺失。

### 迭代 38：流式工具执行

**状态**: 规划中

**问题**: 当前 `collect_response_from_stream()` 等待完整 API 响应后，才由 `execute_tool_uses()` 统一执行所有工具。在模型生成多个工具调用的场景下（如连续 3 个 FileRead），第一个工具块可能在响应开始后数秒就已完整，但仍需等待最后一个工具块完成后才开始执行。原版使用 `StreamingToolExecutor` 在工具块完成后立即执行。

**目标**: 实现流式工具执行器，工具块在流中完成后立即执行，同时保证并发安全和结果顺序。

**产出**:

- `sdk` crate — `streaming_tool_executor.rs`（新文件）:
  - `StreamingToolExecutor` 结构：
    - `new(tool_registry, permission_ui, hook_runner, app_state)` — 构造
    - `add_tool(tool_use_block: ContentBlock) -> bool` — 收到完整 tool_use 块时调用，返回是否立即开始执行
    - `finish() -> Vec<(String, ToolResult)>` — 等待所有执行中的工具完成，按原始顺序返回结果
    - `discard()` — 取消所有未完成的工具执行（用于流式失败降级）
  - 并发控制逻辑：
    - 维护 `pending_tools: Vec<PendingTool>` 队列
    - concurrent-safe 工具：立即 spawn 到后台 (tokio::spawn)
    - non-concurrent 工具：等待前一个非并发工具完成后执行
    - 结果按接收顺序（非完成顺序）缓冲并返回
  - 中止级联：
    - 共享 `CancellationToken`
    - Bash 工具执行失败时，通过 token 取消所有同级 Bash 子进程
    - 非 Bash 工具（FileRead、WebFetch）失败不级联

- `sdk` crate — `agent_loop.rs`:
  - 重构流式处理循环：
    - 流式响应消费过程中，检测 `ContentBlockStop` 事件
    - 若停止的是 `ToolUse` 块，调用 `executor.add_tool()`
    - 响应完成后（`MessageStop`），调用 `executor.finish()` 收集所有结果
    - 用户中断时（Esc/Ctrl+C），调用 `executor.discard()`
  - 保留非流式降级路径：如果流式处理失败，回退到原有的 `collect → execute` 逻辑

- `tools` crate — `tool.rs`:
  - `Tool` trait 新增 `interrupt_behavior(&self) -> InterruptBehavior`（默认 `Cancel`）
  - `InterruptBehavior` 枚举：`Cancel`（可被用户中断取消）、`Block`（不可中断，等待完成）

**验收标准**:

- 3 个 FileRead 调用：第一个在流式响应进行中即开始执行
- 执行结果按工具出现顺序返回，不因完成顺序不同而乱序
- Bash 工具失败后，同级其他 Bash 工具被取消
- FileRead 失败不影响同级其他工具
- Esc 取消后，`Cancel` 类工具被中止，`Block` 类工具继续完成
- 流式执行失败时，自动降级到完整响应后执行
- 性能测试：3 个 FileRead 的端到端时间 < 原来的 60%

**依赖**: `tokio_util::sync::CancellationToken`

---

### 迭代 39：工具延迟加载

**状态**: 规划中

**问题**: 当前所有注册工具的完整 JSON Schema 都在每次 API 请求中发送。随着 MCP 工具和内置工具增加，工具定义占用大量 prompt 空间。原版通过 `shouldDefer` 标记低频工具，仅发送名称，模型按需通过 `ToolSearchTool` 加载完整 schema。

**目标**: 实现工具延迟加载，减少 prompt 空间占用，同时保证模型能按需发现和使用所有工具。

**产出**:

- `tools` crate — `tool.rs`:
  - `Tool` trait 新增 `should_defer(&self) -> bool`（默认 `false`）
  - 以下工具标记为 `should_defer = true`:
    - 所有 MCP 代理工具（`McpProxyTool`）
    - `WebSearchTool`, `WebFetchTool`
    - `LspTool`
    - `TaskTool` (get/list/update/stop 子命令)
    - `ExitPlanModeTool`
    - `NotebookEditTool`
    - `MonitorTool`

- `tools` crate — `tool_search.rs`（新文件）:
  - `ToolSearchTool` 实现：
    - Input: `{ query: String, max_results: Option<usize> }`
    - 搜索逻辑：
      - `"select:ToolName"` 精确选择
      - 关键字搜索：对工具名（CamelCase 拆分 + MCP 名称拆分 `mcp__server__tool`）和描述进行词边界匹配
      - 评分：名称匹配权重 2x, 描述匹配权重 1x
    - 输出：匹配工具的完整 JSON Schema 定义（最多 `max_results` 个，默认 5）
    - `is_read_only = true`, `should_defer = false`（自身不能被延迟！）

- `tools` crate — `registry.rs`:
  - `ToolRegistry` 新增方法：
    - `get_deferred_tools() -> Vec<&ToolInfo>` — 返回所有延迟工具的名称列表（不含 schema）
    - `get_non_deferred_tools() -> Vec<&ToolInfo>` — 返回非延迟工具的完整信息
    - `search_tools(query: &str, max: usize) -> Vec<&ToolInfo>` — 搜索延迟工具
  - `estimate_deferred_schema_tokens() -> usize` — 估算延迟工具 schema 的总 token 数

- `api` crate — `types.rs`:
  - `ApiTool` 新增 `deferred: bool` 字段
  - 构建请求时：非延迟工具发送完整 schema，延迟工具仅发送 `{ name, deferred: true }`

- `sdk` crate — `agent_loop.rs`:
  - 已发现工具追踪：`discovered_tools: HashSet<String>`
  - 每轮开始时，已发现的延迟工具自动升级为非延迟（发送完整 schema）
  - 自动阈值：当延迟工具 schema 总 token 估算超过上下文窗口的 10% 时才启用延迟

**验收标准**:

- 注册 20 个 MCP 工具时，初始请求仅包含核心工具的完整 schema
- `ToolSearchTool` 通过关键字可找到延迟工具并返回完整 schema
- `select:GrepTool` 精确返回 GrepTool 的 schema
- 模型使用 ToolSearchTool 发现延迟工具后，后续轮次自动包含该工具完整 schema
- 延迟工具少于阈值时，所有工具正常发送（不启用延迟）
- 单元测试覆盖搜索评分逻辑

**依赖**: 无

---

### 迭代 40：成本追踪 + 设置迁移 + Hook 补全

**状态**: 规划中

**问题**: 三个独立但都不大的功能缺失：(1) 成本追踪使用硬编码费率，不区分模型和缓存 token；(2) 配置格式变更只能靠 `#[serde(default)]`，无法做复杂迁移；(3) Hook 系统缺少 SessionStart/SessionEnd 事件和 once 标志。

**目标**: 补齐这三个功能点。

**产出**:

- `core` crate — `cost.rs`（新文件）:
  - `ModelPricing` 结构：每百万 token 的 USD 价格
    - `input_per_million`, `output_per_million`
    - `cache_read_per_million`, `cache_creation_per_million`
  - `get_pricing(model: &str) -> ModelPricing` — 按模型名匹配定价:
    - opus 系列: $15/$75 input/output, $1.5 cache_read, $18.75 cache_creation
    - sonnet 系列: $3/$15 input/output, $0.3 cache_read, $3.75 cache_creation
    - haiku 系列: $0.25/$1.25 input/output, $0.03 cache_read, $0.3 cache_creation
    - 未知模型: 使用 sonnet 定价作为默认
  - `calculate_cost(usage: &Usage, model: &str) -> f64` — 计算单次 API 调用成本
  - `CostTracker` 结构：
    - 累计总成本、逐轮成本记录
    - `max_budget_usd: Option<f64>` — 预算限制
    - `check_budget() -> BudgetStatus` — 返回 `Ok`/`Warning(remaining)`/`Exceeded`
  - 集成到 `/cost` 斜杠命令：显示分模型、分缓存类型的详细成本

- `core` crate — `migration.rs`（新文件）:
  - `Migration` trait: `fn version(&self) -> u32`, `fn description(&self) -> &str`, `fn migrate(&self, config: &mut serde_json::Value) -> Result<()>`
  - `MigrationRunner`:
    - `register(migration: Box<dyn Migration>)` — 注册迁移
    - `run_pending(config_path: &Path) -> Result<()>` — 读取当前版本号，运行所有待执行迁移
    - 版本号存储在 config.json 的 `_migration_version` 字段
  - 初始迁移列表：
    - V1: 无操作（基线）
    - V2: `model` 字段中 "claude-3-opus" 重命名为 "claude-opus-4-0"（示例迁移）
  - 启动时在配置加载后、使用前执行迁移

- `sdk` crate — `hooks.rs`:
  - `HookEvent` 新增 `SessionStart` 和 `SessionEnd` 变体
  - `SessionStart` 输入：`{ session_id, cwd, model, permission_mode }`
  - `SessionEnd` 输入：`{ session_id, duration_secs, total_cost_usd, messages_count }`
  - `HookConfig` 新增 `once: bool` 字段（默认 false）
  - `HookRunner` 维护 `executed_once: HashSet<String>`，once=true 的 hook 只执行一次

- `cli` crate — `main.rs`:
  - 会话开始时触发 `SessionStart` hook
  - 会话结束时（正常退出/Ctrl+C）触发 `SessionEnd` hook
  - 预算超出时通过 `OutputSink` 警告用户

- `core` crate — `hooks.rs`:
  - `HookEvent` 枚举增加 `SessionStart`, `SessionEnd` 变体
  - `HookConfig` 增加 `once` 字段

**验收标准**:

- `/cost` 显示分模型定价（opus/sonnet/haiku 费率不同）
- 缓存 token 按差异化费率计算（cache_read 比 input 便宜）
- `maxBudgetUsd: 1.0` 配置后，累计超过 $1 时显示警告
- 设置迁移在启动时自动运行，版本号递增
- 新增迁移脚本后，旧配置自动升级
- SessionStart hook 在会话开始时触发，stdin 包含正确 JSON
- SessionEnd hook 在退出时触发，包含 duration 和 cost
- `once: true` 的 hook 在会话中只触发一次

**依赖**: 无

---

## 4. 阶段 C：生态与清扫（迭代 41-43）

> 目标：补齐认证和指令系统，完善 SDK 和沙箱，清扫所有遗留项。

### 迭代 41：OAuth 认证 + @include 指令

**状态**: 规划中

**问题**: 当前认证仅支持 API key 和 Bearer token，无法通过 claude.ai 账号登录。CLAUDE.md 不支持 `@include` 指令，无法模块化管理项目指令。

**目标**: 实现 OAuth 认证流程和 CLAUDE.md @include 引用。

**产出**:

- `core` crate — `oauth.rs`（新文件）:
  - `OAuthClient` 结构：
    - `start_auth_flow() -> (auth_url, state, pkce_verifier)` — 启动 OAuth 授权码流程
    - `exchange_code(code, verifier) -> TokenPair` — 用授权码换取 access_token + refresh_token
    - `refresh_token(refresh_token) -> TokenPair` — 刷新过期 token
  - `TokenStorage`:
    - 存储到 `~/.config/rust-claude-code/oauth_tokens.json`
    - token 加密存储（可选，平台相关）
  - `TokenPair`: `access_token`, `refresh_token`, `expires_at`
  - OAuth 参数：从 Anthropic 官方 OAuth endpoints 配置

- `cli` crate — `main.rs`:
  - `/login` 命令增强：
    - 启动本地 HTTP 服务器监听回调（`127.0.0.1:{random_port}`）
    - 打开浏览器跳转到 auth URL
    - 接收授权码，换取 token
    - 存储到 TokenStorage
  - `/logout` 命令增强：清除存储的 OAuth tokens
  - 认证优先级调整：config api_key > ANTHROPIC_API_KEY > OAuth token > ANTHROPIC_AUTH_TOKEN > apiKeyHelper

- `api` crate — `client.rs`:
  - 支持 OAuth token 作为 Bearer auth
  - 请求前检查 token 过期，自动 refresh
  - refresh 失败时提示用户重新 `/login`

- `core` crate — `claude_md.rs`:
  - `@include` 指令解析：
    - 语法：`@include path/to/file.md`（单独一行）
    - 路径解析：相对于当前 CLAUDE.md 文件所在目录
    - 递归深度限制：最大 5 层
    - 循环引用检测：维护已访问路径集合
    - 文件不存在时：跳过并输出警告（不中断加载）
  - 内容截断：总字符数超过 `MAX_CLAUDE_MD_CHARS`（40,000）时截断

**验收标准**:

- `/login` 打开浏览器，完成授权后自动获取 token
- token 过期后自动 refresh，无需用户干预
- `/logout` 清除所有存储的 token
- `@include common/rules.md` 正确引入文件内容
- 循环引用 (A includes B, B includes A) 不导致无限递归
- 超过 5 层嵌套时停止递归并警告
- 总内容超过 40K 字符时截断

**依赖**: `tiny_http` 或类似轻量 HTTP 服务器（用于 OAuth 回调）

---

### 迭代 42：沙箱隔离 + SDK 公共 API

**状态**: 规划中

**问题**: Bash 工具直接通过 `tokio::process::Command` 执行，无任何隔离。恶意或意外的命令可能访问任意文件系统和网络。SDK crate 已存在但缺少干净的公共 API，第三方无法嵌入使用。

**目标**: 实现平台级沙箱和可嵌入的 SDK API。

**产出**:

- `core` crate — `sandbox.rs`（新文件）:
  - `SandboxConfig`:
    - `enabled: bool`（默认 false）
    - `allowed_paths: Vec<PathBuf>`（允许读写的路径列表，默认包含项目根目录和 home/.config）
    - `network: NetworkPolicy`（Allow / Deny / AllowList(Vec<String>)）
  - `SandboxAdapter` trait:
    - `wrap_command(cmd: &mut Command, config: &SandboxConfig)` — 修改 Command 以添加沙箱限制
    - `is_available() -> bool` — 检测当前平台是否支持沙箱

- `tools` crate — `sandbox/`（新目录）:
  - `macos.rs`: macOS `sandbox-exec` 适配器
    - 生成 `.sb` profile 文件，限制文件系统访问范围
    - 可选禁止网络 (`(deny network*)`)
  - `linux.rs`: Linux `bubblewrap` (bwrap) 适配器
    - 使用 `--ro-bind`, `--bind`, `--unshare-net` 等参数
    - 检测 bwrap 是否已安装
  - `noop.rs`: 无操作适配器（Windows 或未安装工具时）

- `tools` crate — `bash.rs`:
  - 执行前调用 `SandboxAdapter::wrap_command()` 修改 Command
  - 沙箱不可用时：正常执行（降级），但日志警告

- `cli` crate:
  - `--sandbox` CLI 参数启用沙箱
  - `--sandbox-no-network` 禁止网络访问
  - 配置文件 `sandbox` 段落

- `sdk` crate — `lib.rs` 重构:
  - 公共 API 入口：
    ```rust
    pub struct Session { /* ... */ }

    impl Session {
        pub fn builder() -> SessionBuilder;
        pub async fn send(&self, prompt: &str) -> Result<ResponseStream>;
        pub async fn send_with_tools(&self, prompt: &str, tool_results: Vec<ToolResult>) -> Result<ResponseStream>;
    }

    pub struct SessionBuilder {
        // model, api_key, system_prompt, tools, permission_mode, hooks, etc.
    }

    pub struct ResponseStream { /* ... */ }
    // implements Stream<Item = ResponseEvent>

    pub enum ResponseEvent {
        TextDelta(String),
        ThinkingDelta(String),
        ToolUse { id: String, name: String, input: Value },
        ToolResult { id: String, result: ToolResult },
        Usage(Usage),
        Error(Error),
        Done,
    }
    ```
  - 自定义工具注入：`SessionBuilder::with_tool(tool: Box<dyn Tool>)`
  - Headless 模式：使用 `NoopOutputSink` + `DenyAllPermissionUI`
  - 示例：`examples/sdk_basic.rs`

**验收标准**:

- macOS: `--sandbox` 模式下 Bash 命令无法访问 `/etc/passwd`（非允许路径）
- Linux: bwrap 可用时，沙箱正常工作；不可用时降级并警告
- `--sandbox-no-network` 模式下 `curl` 命令失败
- `Session::builder().model("...").api_key("...").build()` 可构建会话
- `session.send("hello").await` 返回可消费的 `ResponseStream`
- `examples/sdk_basic.rs` 可独立编译运行
- 自定义工具可通过 `with_tool()` 注入并被模型使用

**依赖**: 无新 crate 依赖（sandbox 使用系统命令）

---

### 迭代 43：遗留项清扫

**状态**: 规划中

**问题**: Phase 2-3 遗留了若干小项。单独不值得一个迭代，但累积影响可用性。

**目标**: 清扫所有遗留项，达到计划 100% 完成。

**产出**:

- **TUI Task 视图面板**（迭代 20 遗留）:
  - 右侧 30 列 Task 面板（类似 Todo 面板）
  - 显示所有 Task 的 ID、状态、优先级、简要描述
  - Ctrl+T 切换 Todo/Task 面板
  - 实时刷新（Task 状态变化时通过 AppEvent 通知）

- **WebSearch 多后端**（迭代 26 遗留）:
  - `TavilySearchBackend`: Tavily Search API 集成
  - `SearxngSearchBackend`: SearXNG 自托管实例支持
  - `WebSearchConfig` 增加 `provider` 字段选择后端
  - 环境变量：`TAVILY_API_KEY`, `SEARXNG_URL`

- **TypeScript 语法高亮改进**（迭代 24 遗留）:
  - 为 syntect 加载 TypeScript/TSX 语法定义
  - 如 syntect 默认不含 TS，嵌入 `.sublime-syntax` 文件
  - 测试：TypeScript 类型注解正确着色

- **PermissionMode::Auto 基础版**（迭代 33 遗留）:
  - 新增 `PermissionMode::Auto` 变体
  - 基本策略：读操作自动允许、已知安全命令（git status, ls, cat 等）自动允许
  - 危险操作（rm, chmod, 网络命令等）仍需确认
  - `--mode auto` CLI 参数

- **插件 install/remove 完善**（迭代 34 遗留）:
  - `/plugin install <path>` — 从本地路径安装插件（复制到 ~/.claude/plugins/）
  - `/plugin remove <name>` — 删除已安装插件
  - 安装后自动 reload
  - 插件版本校验（manifest 中的 `version` 字段）

**验收标准**:

- Ctrl+T 可切换到 Task 面板，显示当前所有 Task
- WebSearch 可配置使用 Tavily 或 SearXNG
- TypeScript 代码块有正确的类型注解着色（与 JS 不同）
- `--mode auto` 下 `git status` 自动执行，`rm -rf /` 仍需确认
- `/plugin install ./my-plugin` 正确复制并加载
- `/plugin remove my-plugin` 正确删除

**依赖**: 可能需要 TypeScript sublime-syntax 文件

---

## 5. 阶段验收清单

### Stage A 验收（韧性与安全）
- [ ] 超长上下文 API 错误时自动压缩恢复
- [ ] 连续模型过载时自动降级到备用模型
- [ ] 首次运行显示信任对话框，阻止未信任状态下的 apiKeyHelper 执行
- [ ] 文件被外部修改后 FileEdit 拒绝覆盖
- [ ] 会话以 JSONL 增量写入，崩溃后可恢复

### Stage B 验收（性能与补齐）
- [ ] 多工具调用时，第一个工具在流式响应中即开始执行
- [ ] 大量 MCP 工具时，初始请求 prompt 显著减少
- [ ] `/cost` 显示分模型、分缓存类型的精确成本
- [ ] 设置格式变更时自动迁移
- [ ] SessionStart/SessionEnd hook 正常触发

### Stage C 验收（生态与清扫）
- [ ] `/login` 可通过 OAuth 登录 claude.ai 账号
- [ ] CLAUDE.md 中 `@include` 正确引入外部文件
- [ ] `--sandbox` 模式下 Bash 命令受限
- [ ] SDK `Session` API 可独立使用
- [ ] 所有 Phase 2-3 遗留项清零

---

## 6. 不在第四期范围的功能

以下功能经评估后决定不纳入 Phase 4：

| 功能 | 原因 |
|------|------|
| Vim 模式 | 实现复杂度高，用户群小，优先级低 |
| 语音模式 | 依赖平台 API，实现复杂，非核心功能 |
| 远程控制 (Bridge/CCR) | 需要独立基础设施，不适合单人维护 |
| Computer Use (屏幕控制) | 非 CLI 工具核心场景 |
| Swarm 编排 (多代理协作) | 等自定义 Agent 稳定后再评估 |
| 功能开关系统 (GrowthBook 等) | 单人项目不需要 A/B 测试基础设施 |
| 分析/遥测 (Analytics) | 隐私考虑，暂不引入 |
| Git Worktree 工具 | Bash 可替代，低频需求 |
| 插件市场 | 等插件系统稳定后再评估 |
| MDM / 远程托管设置 | 企业功能，暂不需要 |
| Prompt 缓存中断检测 | 调试工具，低优先级 |
| 结构化输出工具 | 使用场景有限 |

---

## 7. 迭代依赖关系

```
迭代 35 (响应式压缩+模型降级) ← 无依赖
迭代 36 (信任对话框+文件缓存) ← 无依赖
迭代 37 (JSONL 会话) ← 无依赖

迭代 38 (流式工具执行) ← 建议在 35 之后（重构 agent_loop）
迭代 39 (工具延迟加载) ← 无强依赖
迭代 40 (成本/迁移/Hook) ← 无依赖

迭代 41 (OAuth + @include) ← 无依赖
迭代 42 (沙箱 + SDK) ← 无依赖
迭代 43 (遗留清扫) ← 建议在其他迭代之后
```

Stage A 的三个迭代可并行开发。Stage B 中迭代 38 建议在 35 之后（因为都修改 agent_loop）。迭代 43 建议放在最后。