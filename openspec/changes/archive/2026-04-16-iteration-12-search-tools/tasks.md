## 1. Dependencies & Setup

- [x] 1.1 Add `glob`, `walkdir`, and `regex` crate dependencies to `crates/tools/Cargo.toml`
- [x] 1.2 Add `glob` and `grep` module declarations to `crates/tools/src/lib.rs` with public exports

## 2. GlobTool Implementation

- [x] 2.1 Create `crates/tools/src/glob.rs` with `GlobTool` struct, `GlobInput` deserialization, and `Tool` trait implementation (`info()`, `is_read_only() = true`, `is_concurrency_safe() = true`, `execute()`)
- [x] 2.2 Implement glob search logic: resolve search root from `path` param or CWD, run glob pattern, collect matching file paths
- [x] 2.3 Implement result sorting by modification time (newest first) and format output as one path per line
- [x] 2.4 Add unit tests for GlobTool: basic pattern matching, custom root path, no matches returns empty, mtime sorting, missing pattern error

## 3. GrepTool Implementation

- [x] 3.1 Create `crates/tools/src/grep.rs` with `GrepTool` struct, `GrepInput` deserialization (all fields: pattern, path, glob, type, output_mode, head_limit, context line params, case-insensitive flag), and `Tool` trait implementation
- [x] 3.2 Implement file type mapping: define a mapping from type shorthand (`rs`, `py`, `js`, `ts`, `go`, `java`, etc.) to file extensions
- [x] 3.3 Implement directory walking with file filtering: use `walkdir` for traversal, filter by glob pattern and/or type, skip binary files and hidden directories
- [x] 3.4 Implement `files_with_matches` output mode: search each file for regex match, collect and return matching file paths
- [x] 3.5 Implement `content` output mode: return matching lines with `path:line:content` format, support context lines (-A, -B, -C), support case-insensitive flag
- [x] 3.6 Implement `head_limit` enforcement (default 250) to cap output entries
- [x] 3.7 Add unit tests for GrepTool: basic search, regex search, files_with_matches mode, content mode, context lines, file type filter, glob filter, case-insensitive, head_limit, no matches, missing pattern error

## 4. Integration

- [x] 4.1 Register `GlobTool` and `GrepTool` in `crates/cli/src/main.rs` tool registry setup
- [x] 4.2 Add `Glob` and `Grep` tool descriptions to the system prompt builder in `crates/cli/src/system_prompt.rs`
- [x] 4.3 Update the `test_register_all_core_tools` test in `crates/tools/src/registry.rs` to include both new tools

## 5. Verification

- [x] 5.1 Run `cargo check --workspace` — all crates compile cleanly
- [x] 5.2 Run `cargo test --workspace` — all existing and new tests pass
- [ ] 5.3 Manual smoke test: run `cargo run -p rust-claude-cli -- "list all Rust source files in this project"` and verify GlobTool is invoked
