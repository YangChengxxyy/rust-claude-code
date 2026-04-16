## Why

The current Rust Claude Code only has 5 core tools (Bash, FileRead, FileEdit, FileWrite, TodoWrite). Users exploring codebases must fall back to `BashTool` for running `find`/`grep` commands, which is slower, less structured, and bypasses the permission system's granularity. Adding dedicated `GlobTool` and `GrepTool` brings the tool set closer to the original Claude Code, significantly improving code exploration efficiency and enabling the LLM to discover files and search content with purpose-built, permission-aware tools.

## What Changes

- Add `GlobTool` to the `tools` crate: fast file pattern matching using glob patterns (e.g., `**/*.rs`, `src/**/*.ts`), returning sorted matching file paths
- Add `GrepTool` to the `tools` crate: content search powered by regex, supporting path/glob/type filtering, context lines, output modes (file paths only vs. matching content), and result limiting
- Register both tools in `ToolRegistry` alongside existing tools in `cli/src/main.rs`
- Add tool descriptions to the system prompt builder in `cli/src/system_prompt.rs`
- Both tools are read-only and concurrency-safe, fitting into the existing tool concurrency model

## Capabilities

### New Capabilities
- `glob-tool`: File pattern matching tool that searches for files by glob patterns, with configurable search root and sorted results
- `grep-tool`: Content search tool that searches file contents by regex pattern, with filtering by path/glob/type, context lines, multiple output modes, and result limiting

### Modified Capabilities
_(none — no existing spec-level requirements change)_

## Impact

- **`crates/tools/`**: Two new source files (`glob.rs`, `grep.rs`), updated `lib.rs` exports and module declarations
- **`crates/tools/Cargo.toml`**: New dependency on `glob` crate (for GlobTool) and `regex` crate (for GrepTool); may also use `ignore` crate for `.gitignore`-aware walking
- **`crates/cli/src/main.rs`**: Register `GlobTool` and `GrepTool` in the tool registry
- **`crates/cli/src/system_prompt.rs`**: Add tool descriptions for GlobTool and GrepTool
- **Permission system**: Both tools are read-only, so they will be auto-allowed in all modes except `plan` (which blocks nothing for read-only tools). No permission rule changes needed.
- **No breaking changes** to existing tools or APIs
