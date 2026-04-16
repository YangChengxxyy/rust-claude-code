## 1. CLAUDE.md 文件发现模块

- [x] 1.1 在 `core` crate 新增 `claude_md` 模块，定义 `ClaudeMdFile` 结构体（path + content）和 `discover_claude_md(cwd: &Path) -> Vec<ClaudeMdFile>` 公开函数
- [x] 1.2 实现 git root 检测辅助函数：从给定目录向上查找包含 `.git` 的目录，返回 git repo 根路径
- [x] 1.3 实现向上遍历逻辑：从 CWD 开始沿父目录向上，到 git root 或文件系统根为止，收集所有 `CLAUDE.md` 文件路径
- [x] 1.4 实现全局 CLAUDE.md 发现：检查 `~/.claude/CLAUDE.md`（或 `$CLAUDE_CONFIG_DIR/CLAUDE.md`）
- [x] 1.5 实现结果排序：全局文件在前，项目文件按 root-most 到 leaf-most 排列
- [x] 1.6 实现错误处理：文件权限不足时跳过、符号链接解析避免循环
- [x] 1.7 为 `claude_md` 模块编写单元测试：覆盖单文件、多文件、无文件、git root 边界、全局文件等场景

## 2. System Prompt 注入

- [x] 2.1 在 `cli/system_prompt.rs` 修改 `build_system_prompt()` 签名，新增 `claude_md_files: &[ClaudeMdFile]` 参数
- [x] 2.2 实现 `build_claude_md_section()` 函数：将多个 ClaudeMdFile 合并为带源路径注释的 `# claudeMd` section
- [x] 2.3 实现截断逻辑：总内容超过 30000 字符时，从全局/root 文件开始截断，保留 leaf-most 内容，并附加截断提示
- [x] 2.4 将 claudeMd section 插入 system prompt 正确位置（环境信息之后、custom append 之前）
- [x] 2.5 为 system prompt 注入编写单元测试：覆盖无文件、单文件、多文件合并、截断、section 顺序等场景

## 3. CLI 集成

- [x] 3.1 在 `cli/main.rs` 启动流程中调用 `discover_claude_md()`，将结果传入 `build_system_prompt()`
- [x] 3.2 确保非交互模式（`--print`）和交互模式都正确注入 CLAUDE.md
- [x] 3.3 在 verbose 模式下输出发现的 CLAUDE.md 文件列表用于调试

## 4. 验证与集成测试

- [x] 4.1 运行 `cargo test --workspace` 确认所有现有测试通过
- [x] 4.2 运行 `cargo check --workspace` 确认无编译警告
- [x] 4.3 手动验证：在有 CLAUDE.md 的项目中运行 `cargo run -p rust-claude-cli -- --verbose "test"` 确认 system prompt 包含项目指令
