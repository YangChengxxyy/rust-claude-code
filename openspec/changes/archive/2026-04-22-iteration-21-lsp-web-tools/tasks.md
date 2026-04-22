## 1. LSP 基础设施与会话管理

- [x] 1.1 在 `tools` crate 中新增 LSP 相关模块（协议类型、请求封装、stdio 传输）
- [x] 1.2 实现最小 JSON-RPC over stdio 客户端，支持 LSP initialize 和基础请求/响应匹配
- [x] 1.3 实现语言到服务端命令的发现规则（Rust / TypeScript / Python）
- [x] 1.4 实现按需启动与复用 LSP 会话的管理器
- [x] 1.5 为 LSP 传输、初始化和服务端发现补充单元测试

## 2. LspTool 实现

- [x] 2.1 新增 `LspTool`，定义 `goToDefinition`、`findReferences`、`hover`、`documentSymbol`、`workspaceSymbol` 输入 schema
- [x] 2.2 实现 `goToDefinition` 请求与结果格式化
- [x] 2.3 实现 `findReferences` 请求与结果格式化
- [x] 2.4 实现 `hover` 请求与结果格式化
- [x] 2.5 实现 `documentSymbol` 和 `workspaceSymbol` 请求与结果格式化
- [x] 2.6 处理不支持语言、服务端启动失败、LSP 请求失败等错误场景
- [x] 2.7 为 `LspTool` 的各操作补充单元测试
- [x] 2.8 验证 `cargo test -p rust-claude-tools` 通过

## 3. WebFetchTool 实现

- [x] 3.1 在合适 crate 中新增网页获取与 HTML 提取辅助模块
- [x] 3.2 新增 `WebFetchTool`，定义 `url` 与可选 `prompt` 输入 schema
- [x] 3.3 实现 HTTP GET 获取网页内容并抽取可读正文
- [x] 3.4 实现网页内容转 Markdown/纯文本输出
- [x] 3.5 实现基于 URL 的短期缓存（15 分钟 TTL）
- [x] 3.6 实现大页面内容截断逻辑
- [x] 3.7 为获取失败、缓存命中、截断等场景补充测试
- [x] 3.8 验证 `cargo test -p rust-claude-tools` 通过

## 4. WebSearchTool 实现

- [x] 4.1 定义搜索后端抽象接口与配置结构
- [x] 4.2 新增 `WebSearchTool`，定义 `query`、`allowed_domains`、`blocked_domains` 输入 schema
- [x] 4.3 实现首个搜索后端适配（如 SearXNG 或 Brave 的单后端实现）
- [x] 4.4 实现搜索结果结构化格式化（title / url / summary）
- [x] 4.5 实现允许/阻止域名过滤逻辑
- [x] 4.6 处理搜索后端错误与无结果场景
- [x] 4.7 为搜索结果格式化、域名过滤和失败场景补充测试
- [x] 4.8 验证 `cargo test -p rust-claude-tools` 通过

## 5. CLI 与 ToolRegistry 集成

- [x] 5.1 在 `build_tools()` 中注册 `LspTool`、`WebFetchTool`、`WebSearchTool`
- [x] 5.2 确认 system prompt 自动暴露三个新工具描述
- [x] 5.3 确认 `--allowed-tools` / `--disallowed-tools` 对三个新工具正常生效
- [x] 5.4 为 CLI 层工具注册与过滤场景补充测试
- [x] 5.5 验证 `cargo test -p rust-claude-cli` 通过

## 6. 集成验证与文档更新

- [x] 6.1 运行 `cargo test --workspace` 确认全仓通过
- [x] 6.2 运行 `cargo build` 确认编译通过
- [x] 6.3 验证 `LspTool` 在 Rust 项目内可完成至少一次 `goToDefinition` 集成测试
- [x] 6.4 验证 `WebFetchTool` 可成功抓取并返回网页内容
- [x] 6.5 验证 `WebSearchTool` 可返回格式化搜索结果
- [x] 6.6 更新 `doc/requirement.md` 中迭代 21 的状态
