## Context

迭代 21 的目标是为 Rust Claude Code 增加两类缺失的高价值工具：代码语义导航（LSP）和外部信息获取（WebFetch/WebSearch）。当前仓库已经具备适合承载这些能力的基础设施：

- `tools` crate 已有统一 `Tool` trait 和 ToolRegistry
- `query_loop` 已具备权限检查、工具执行、system prompt 注入
- `mcp` crate 已实现 JSON-RPC over stdio，可复用部分协议/传输实现思路
- `glob` / `grep` 已覆盖文本级代码检索，但无法提供定义跳转、引用查找等语义能力

这三个新工具都跨越多个模块：工具实现、外部进程/HTTP 生命周期、权限系统、测试与 prompt 暴露，因此需要明确设计边界。

## Goals / Non-Goals

**Goals:**
- 提供 `LspTool`，支持 `goToDefinition`、`findReferences`、`hover`、`documentSymbol`、`workspaceSymbol`
- 支持自动发现并启动常见语言的 LSP 服务端（Rust / TypeScript / Python）
- 提供 `WebFetchTool`，能够获取网页并转换成适合模型消费的文本/Markdown
- 提供 `WebSearchTool`，能够通过可配置搜索后端返回结构化搜索结果
- 保持所有新工具与现有权限系统、ToolRegistry、QueryLoop、system prompt 兼容

**Non-Goals:**
- 不实现完整浏览器渲染或 JavaScript 执行型抓取
- 不实现完整 LSP 会话共享/后台守护进程池
- 不实现复杂搜索后端聚合或多搜索引擎并行
- 不实现与 MCP 的深度集成（本迭代的 Web/LSP 工具是原生工具）

## Decisions

### D1: LSP 复用“JSON-RPC over stdio”模式，但不直接依赖 mcp crate API

**选择**: `LspTool` 采用与 MCP 类似的 JSON-RPC over stdio 通信模式，但在 `tools` crate 内部实现最小 LSP 客户端层，必要时抽取共享辅助模块，而不是直接把 LSP 建在 `mcp` crate API 之上。

**理由**:
- MCP 与 LSP 协议形态相似，但请求/响应结构、生命周期与语义完全不同
- 直接耦合会把 `mcp` crate 变成通用 RPC 层，扩大职责边界
- 可复用设计经验，但不强行共用概念模型

**备选方案**:
- 直接复用 `mcp` crate：短期省代码，长期协议边界会混乱
- 引入第三方 Rust LSP 客户端 SDK：生态不稳定，且难以按仓库风格做最小可控实现

### D2: LSP 服务端按语言“按需启动”，不做全局后台池

**选择**: `LspTool` 在调用时根据文件扩展名/工作区语言判断所需服务端，启动对应 LSP 进程并复用当前会话内的连接；首期不做跨会话后台池。

**理由**:
- 实现复杂度可控
- 符合当前 CLI 生命周期短、单会话执行的特点
- 避免处理后台进程僵尸管理、重连和共享状态

**备选方案**:
- 程序启动时预热所有支持语言的 LSP：浪费资源，用户项目可能只需要一种语言
- 全局后台池：收益不明显，但复杂度很高

### D3: WebFetch 首期使用普通 HTTP 获取 + HTML 提取，不执行 JS

**选择**: `WebFetchTool` 基于普通 HTTP GET 获取网页内容，并将 HTML 转换为 Markdown/文本；首期不执行 JavaScript。

**理由**:
- 与用户“获取文档/博客/静态页面内容”的主流需求匹配
- 不引入浏览器级依赖，能保持工具轻量
- 容易与缓存、截断策略结合

**备选方案**:
- 引入 headless browser：功能更强，但成本、依赖和测试复杂度明显上升

### D4: WebFetch 与 WebSearch 都采用结果截断和格式化输出

**选择**: 两个 Web 工具都返回适合模型消费的紧凑文本，而不是原始响应体。`WebFetch` 支持按提示提取内容并截断；`WebSearch` 返回标题、URL、摘要列表。

**理由**:
- 与现有 `ToolResult` 的文本模型一致
- 控制 token 消耗，避免网页全文直接塞给模型
- 提升工具结果可读性和稳定性

### D5: 搜索后端用可配置抽象，首期实现单后端适配

**选择**: `WebSearchTool` 定义后端抽象接口，配置层可指定 Brave Search、SearXNG 或兼容后端。首期只需要落地一个默认后端实现。

**理由**:
- 避免把具体供应商绑定进工具接口
- 为后续替换/自托管留出口

## Risks / Trade-offs

**[LSP 进程兼容性风险]** → 不同语言服务端的安装路径、启动命令、初始化参数可能不同。  
→ **缓解**: 首期按语言内置最小发现规则，并允许配置覆盖。

**[Web 内容提取质量风险]** → 普通 HTML 转 Markdown 可能带来噪声，影响模型消费质量。  
→ **缓解**: 首期增加基础正文提取、截断和 prompt 驱动提取策略。

**[搜索供应商依赖风险]** → 某个搜索 API 不可用会影响 WebSearch。  
→ **缓解**: 通过后端抽象和配置保留替换能力。

**[权限边界风险]** → Web 工具虽然逻辑上偏只读，但仍涉及网络外联。  
→ **缓解**: 首期统一按非只读工具处理，走现有权限系统，后续再细化安全策略。
